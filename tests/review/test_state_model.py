import copy
import hashlib
import struct
import unittest
from dataclasses import dataclass, field
from typing import Optional


MAX_MEMBERS = 256
MAX_TREE_DEPTH = 8
DISTRIBUTION_MAGIC = b"ASTRAD01"
GROUP_MAGIC = b"ASTRAM01"
DEFAULT_OWNER = bytes(32)
PROGRAM_ID = bytes([0x11]) * 32
STATE_ID = bytes([0x22]) * 32


class ModelError(Exception):
    def __init__(self, code):
        super().__init__(code)
        self.code = code


def le_u32(value):
    return struct.pack("<I", value)


def le_u64(value):
    return struct.pack("<Q", value)


def le_u128(value):
    return value.to_bytes(16, "little")


def le_i64(value):
    return struct.pack("<q", value)


def hash_parts(domain, parts):
    h = hashlib.sha256()
    h.update(le_u64(len(domain)))
    h.update(domain)
    for part in parts:
        h.update(le_u64(len(part)))
        h.update(part)
    return h.digest()


def context(program_id, state_id):
    return hash_parts(b"astra/context/v1", [program_id, state_id])


def empty_leaf():
    return hash_parts(b"astra/empty/v1", [])


def node_hash(left, right):
    return hash_parts(b"astra/node/v1", [left, right])


def app_nullifier(scope, purpose, sequence, account_id, secret):
    return hash_parts(
        b"astra/nullifier/v1",
        [scope, purpose, le_u64(sequence), account_id, secret],
    )


def review_secret(index):
    return hash_parts(b"review/secret/v1", [le_u64(index)])


def review_vpk(secret):
    return hash_parts(b"review/vpk/v1", [secret])


def review_account_id(secret, viewing_public_key, identifier):
    return hash_parts(
        b"review/account/v1",
        [secret, viewing_public_key, le_u128(identifier)],
    )


def review_salt(index):
    return hash_parts(b"review/salt/v1", [le_u64(index)])


def next_power_of_two(value):
    power = 1
    while power < value:
        power *= 2
    return power


def merkle_levels(leaves):
    count = len(leaves)
    if count == 0 or count > MAX_MEMBERS:
        raise ModelError("InvalidSize")
    padded = list(leaves) + [empty_leaf()] * (next_power_of_two(count) - count)
    levels = [padded]
    while len(padded) > 1:
        padded = [node_hash(padded[i], padded[i + 1]) for i in range(0, len(padded), 2)]
        levels.append(padded)
    return levels


def merkle_root(leaves):
    return merkle_levels(leaves)[-1][0]


def merkle_proof(leaves, index):
    levels = merkle_levels(leaves)
    proof = []
    cursor = index
    for level in levels[:-1]:
        proof.append(level[cursor ^ 1])
        cursor //= 2
    return proof


def verify_membership(root, leaf_hash, count, leaf_index, siblings):
    if count == 0 or count > MAX_MEMBERS:
        return False
    if leaf_index >= count:
        return False
    expected_depth = next_power_of_two(count).bit_length() - 1
    if len(siblings) != expected_depth or len(siblings) > MAX_TREE_DEPTH:
        return False
    cursor = leaf_hash
    index = leaf_index
    for sibling in siblings:
        if index & 1:
            cursor = node_hash(sibling, cursor)
        else:
            cursor = node_hash(cursor, sibling)
        index >>= 1
    return cursor == root


def flip_first_byte(value):
    return bytes([value[0] ^ 0x01]) + value[1:]


@dataclass
class MemberLeaf:
    account_id: bytes
    salt: bytes
    entitlement: int

    def commitment(self, scope):
        return hash_parts(
            b"astra/member/v1",
            [scope, self.account_id, self.salt, le_u64(self.entitlement)],
        )


@dataclass
class MemberWitness:
    leaf: MemberLeaf
    secret: bytes
    viewing_public_key: bytes
    identifier: int
    leaf_index: int
    siblings: list


@dataclass
class Account:
    account_id: bytes
    is_authorized: bool = False
    owner: bytes = DEFAULT_OWNER
    data: bytes = b""
    nonce: int = 0


@dataclass
class DistributionState:
    root: bytes
    member_count: int
    claims: list = field(default_factory=list)


@dataclass
class Proposal:
    sequence: int
    next_value: int
    approvals: list = field(default_factory=list)
    executed: bool = False


@dataclass
class GroupState:
    root: bytes
    member_count: int
    threshold: int
    value: int
    sequence: int = 0
    proposal: Optional[Proposal] = None


def check_owned(state_account):
    if state_account.owner != PROGRAM_ID:
        raise ModelError("WrongOwner")


def verify_witness(root, count, scope, witness, member_account):
    if not member_account.is_authorized:
        raise ModelError("Unauthorized")
    derived = review_account_id(
        witness.secret,
        witness.viewing_public_key,
        witness.identifier,
    )
    if derived != member_account.account_id or witness.leaf.account_id != member_account.account_id:
        raise ModelError("IdentityMismatch")
    if witness.leaf.entitlement <= 0:
        raise ModelError("InvalidMembership")
    if not verify_membership(
        root,
        witness.leaf.commitment(scope),
        count,
        witness.leaf_index,
        witness.siblings,
    ):
        raise ModelError("InvalidMembership")


def distribution_claim(scope, state_account, state, witness, member_account):
    check_owned(state_account)
    verify_witness(state.root, state.member_count, scope, witness, member_account)
    nullifier = app_nullifier(
        scope,
        b"allowlist",
        0,
        witness.leaf.account_id,
        witness.secret,
    )
    if nullifier in state.claims:
        raise ModelError("DuplicateClaim")
    next_state = copy.deepcopy(state)
    next_state.claims.append(nullifier)
    return next_state


def group_create(root, member_count, threshold, initial_value):
    if threshold == 0 or threshold > member_count:
        raise ModelError("InvalidThreshold")
    return GroupState(root=root, member_count=member_count, threshold=threshold, value=initial_value)


def group_propose(scope, state_account, state, witness, member_account, next_value):
    check_owned(state_account)
    verify_witness(state.root, state.member_count, scope, witness, member_account)
    if state.proposal is not None and not state.proposal.executed:
        raise ModelError("PendingProposal")
    next_state = copy.deepcopy(state)
    next_state.sequence += 1
    next_state.proposal = Proposal(sequence=next_state.sequence, next_value=next_value)
    return next_state


def group_approve(scope, state_account, state, witness, member_account):
    check_owned(state_account)
    verify_witness(state.root, state.member_count, scope, witness, member_account)
    if state.proposal is None:
        raise ModelError("NoProposal")
    if state.proposal.executed:
        raise ModelError("AlreadyExecuted")
    nullifier = app_nullifier(
        scope,
        b"threshold",
        state.proposal.sequence,
        witness.leaf.account_id,
        witness.secret,
    )
    if nullifier in state.proposal.approvals:
        raise ModelError("DuplicateApproval")
    if len(state.proposal.approvals) >= state.threshold:
        raise ModelError("ThresholdAlreadyMet")
    next_state = copy.deepcopy(state)
    next_state.proposal.approvals.append(nullifier)
    return next_state


def group_execute(state_account, state):
    check_owned(state_account)
    if state.proposal is None:
        raise ModelError("NoProposal")
    if state.proposal.executed:
        raise ModelError("AlreadyExecuted")
    if len(state.proposal.approvals) < state.threshold:
        raise ModelError("ThresholdNotMet")
    next_state = copy.deepcopy(state)
    next_state.value = next_state.proposal.next_value
    next_state.proposal.executed = True
    return next_state


def encode_hash_vec(values):
    return le_u32(len(values)) + b"".join(values)


def distribution_state_bytes(state):
    return DISTRIBUTION_MAGIC + state.root + le_u32(state.member_count) + encode_hash_vec(state.claims)


def group_state_bytes(state):
    out = GROUP_MAGIC + state.root + le_u32(state.member_count)
    out += le_u32(state.threshold) + le_i64(state.value) + le_u64(state.sequence)
    if state.proposal is None:
        return out + b"\x00"
    proposal = state.proposal
    return (
        out
        + b"\x01"
        + le_u64(proposal.sequence)
        + le_i64(proposal.next_value)
        + encode_hash_vec(proposal.approvals)
        + (b"\x01" if proposal.executed else b"\x00")
    )


def make_fixture(member_count=3):
    scope = context(PROGRAM_ID, STATE_ID)
    witnesses = []
    for index in range(member_count):
        secret = review_secret(index)
        viewing_public_key = review_vpk(secret)
        identifier = 1000 + index
        account_id = review_account_id(secret, viewing_public_key, identifier)
        leaf = MemberLeaf(account_id, review_salt(index), 10 * (index + 1))
        witnesses.append(
            MemberWitness(
                leaf=leaf,
                secret=secret,
                viewing_public_key=viewing_public_key,
                identifier=identifier,
                leaf_index=index,
                siblings=[],
            )
        )
    leaves = [w.leaf.commitment(scope) for w in witnesses]
    root = merkle_root(leaves)
    for index, witness in enumerate(witnesses):
        witness.siblings = merkle_proof(leaves, index)
    members = [Account(w.leaf.account_id, is_authorized=True) for w in witnesses]
    return scope, root, witnesses, members


class StateModelTests(unittest.TestCase):
    def assert_model_error_unchanged(self, expected_code, state, action):
        before = copy.deepcopy(state)
        with self.assertRaises(ModelError) as raised:
            action()
        self.assertEqual(expected_code, raised.exception.code)
        self.assertEqual(before, state)

    def test_pinned_hash_merkle_and_nullifier_vectors(self):
        scope, root, witnesses, _ = make_fixture(3)
        self.assertEqual(
            "129bbc5fcbb2fb216dd0c04e4fb09d8003bc0ff0358ec69834a9b53afa0a71d2",
            hash_parts(b"d", [b"ab", b"c"]).hex(),
        )
        self.assertEqual(
            "b41dffc6ea762880abfdbf5cf9fc05ce994eba6950c357cb3daadd9e273ec668",
            hash_parts(b"d", [b"a", b"bc"]).hex(),
        )
        self.assertNotEqual(
            hash_parts(b"d", [b"ab", b"c"]),
            hash_parts(b"d", [b"a", b"bc"]),
        )
        self.assertEqual(
            "b5424d0d0155d48800c07640f96ee5bcc29c84c8e2e30195f7babd44480597e3",
            scope.hex(),
        )
        self.assertEqual(
            "a27571f402ad3afd0136ca653bc4e2abb3901f0769d22aa7c00d53c1feab64dc",
            root.hex(),
        )
        self.assertEqual(
            [
                "e86e25005e857cc7f95cdf527537abaed5a7a0e78669463c43bd106f1b3a62e7",
                "6f715086618070b2669d0e90384c121473a814968fa6d78a9cbc913241b925cb",
            ],
            [value.hex() for value in witnesses[0].siblings],
        )
        self.assertEqual(
            "c498183f219a24c1d389eb6c3d7dc7487f6ead4a5b841a38672ce553855ebe73",
            app_nullifier(scope, b"allowlist", 0, witnesses[0].leaf.account_id, witnesses[0].secret).hex(),
        )
        self.assertEqual(
            "91fd79572831a02d114b028fc6972efa6f6908c5b9db38131faad22a566dc429",
            app_nullifier(scope, b"threshold", 1, witnesses[0].leaf.account_id, witnesses[0].secret).hex(),
        )
        self.assertEqual(
            "bc6dcd06465a8ef5e5027daf3bb79dc5200d0302035fbfd878d3b61e998b4636",
            app_nullifier(scope, b"threshold", 2, witnesses[0].leaf.account_id, witnesses[0].secret).hex(),
        )

    def test_all_supported_merkle_sizes_verify_and_reject_out_of_range(self):
        for count in [1, 2, 3, 4, 5, 7, 8, 9, 16, 17, 31, 32, 64, 128, 256]:
            scope, root, witnesses, _ = make_fixture(count)
            for witness in witnesses:
                self.assertTrue(
                    verify_membership(
                        root,
                        witness.leaf.commitment(scope),
                        count,
                        witness.leaf_index,
                        witness.siblings,
                    )
                )
            self.assertFalse(
                verify_membership(
                    root,
                    witnesses[0].leaf.commitment(scope),
                    count,
                    count,
                    witnesses[0].siblings,
                )
            )
        with self.assertRaises(ModelError):
            merkle_levels([])
        with self.assertRaises(ModelError):
            merkle_levels([bytes([index % 251]) * 32 for index in range(MAX_MEMBERS + 1)])

    def test_distribution_claim_mutations_do_not_change_state(self):
        scope, root, witnesses, members = make_fixture(3)
        state_account = Account(STATE_ID, owner=PROGRAM_ID)
        state = DistributionState(root=root, member_count=3)

        mutation_cases = []

        tampered_salt = copy.deepcopy(witnesses[0])
        tampered_salt.leaf.salt = flip_first_byte(tampered_salt.leaf.salt)
        mutation_cases.append(("InvalidMembership", tampered_salt, members[0]))

        tampered_path = copy.deepcopy(witnesses[0])
        tampered_path.siblings[0] = flip_first_byte(tampered_path.siblings[0])
        mutation_cases.append(("InvalidMembership", tampered_path, members[0]))

        short_path = copy.deepcopy(witnesses[0])
        short_path.siblings = short_path.siblings[:-1]
        mutation_cases.append(("InvalidMembership", short_path, members[0]))

        bad_index = copy.deepcopy(witnesses[0])
        bad_index.leaf_index = 3
        mutation_cases.append(("InvalidMembership", bad_index, members[0]))

        wrong_secret = copy.deepcopy(witnesses[0])
        wrong_secret.secret = flip_first_byte(wrong_secret.secret)
        mutation_cases.append(("IdentityMismatch", wrong_secret, members[0]))

        wrong_view_key = copy.deepcopy(witnesses[0])
        wrong_view_key.viewing_public_key = flip_first_byte(wrong_view_key.viewing_public_key)
        mutation_cases.append(("IdentityMismatch", wrong_view_key, members[0]))

        wrong_identifier = copy.deepcopy(witnesses[0])
        wrong_identifier.identifier += 1
        mutation_cases.append(("IdentityMismatch", wrong_identifier, members[0]))

        wrong_account = copy.deepcopy(witnesses[0])
        wrong_account.leaf.account_id = witnesses[1].leaf.account_id
        mutation_cases.append(("IdentityMismatch", wrong_account, members[0]))

        zero_entitlement = copy.deepcopy(witnesses[0])
        zero_entitlement.leaf.entitlement = 0
        mutation_cases.append(("InvalidMembership", zero_entitlement, members[0]))

        unauthorized = copy.deepcopy(members[0])
        unauthorized.is_authorized = False
        mutation_cases.append(("Unauthorized", copy.deepcopy(witnesses[0]), unauthorized))

        for expected_code, witness, account in mutation_cases:
            self.assert_model_error_unchanged(
                expected_code,
                state,
                lambda witness=witness, account=account: distribution_claim(
                    scope,
                    state_account,
                    state,
                    witness,
                    account,
                ),
            )

        wrong_owner = Account(STATE_ID, owner=DEFAULT_OWNER)
        self.assert_model_error_unchanged(
            "WrongOwner",
            state,
            lambda: distribution_claim(scope, wrong_owner, state, witnesses[0], members[0]),
        )

        claimed = distribution_claim(scope, state_account, state, witnesses[0], members[0])
        self.assertEqual(1, len(claimed.claims))
        self.assert_model_error_unchanged(
            "DuplicateClaim",
            claimed,
            lambda: distribution_claim(scope, state_account, claimed, witnesses[0], members[0]),
        )

    def test_public_distribution_state_does_not_contain_plain_witness_material(self):
        scope, root, witnesses, members = make_fixture(3)
        state_account = Account(STATE_ID, owner=PROGRAM_ID)
        state = DistributionState(root=root, member_count=3)
        claimed = distribution_claim(scope, state_account, state, witnesses[0], members[0])
        public_bytes = distribution_state_bytes(claimed)

        self.assertIn(root, public_bytes)
        self.assertIn(claimed.claims[0], public_bytes)
        self.assertNotIn(witnesses[0].secret, public_bytes)
        self.assertNotIn(witnesses[0].viewing_public_key, public_bytes)
        self.assertNotIn(witnesses[0].leaf.account_id, public_bytes)
        self.assertNotIn(witnesses[0].leaf.salt, public_bytes)

    def test_group_threshold_lifecycle_and_error_cases(self):
        scope, root, witnesses, members = make_fixture(3)
        state_account = Account(STATE_ID, owner=PROGRAM_ID)
        state = group_create(root, member_count=3, threshold=2, initial_value=7)

        self.assert_model_error_unchanged("NoProposal", state, lambda: group_execute(state_account, state))
        proposed = group_propose(scope, state_account, state, witnesses[0], members[0], next_value=42)
        self.assertEqual(1, proposed.sequence)
        self.assertEqual([], proposed.proposal.approvals)
        self.assert_model_error_unchanged(
            "PendingProposal",
            proposed,
            lambda: group_propose(scope, state_account, proposed, witnesses[1], members[1], next_value=43),
        )
        self.assert_model_error_unchanged(
            "ThresholdNotMet",
            proposed,
            lambda: group_execute(state_account, proposed),
        )

        approved_once = group_approve(scope, state_account, proposed, witnesses[0], members[0])
        self.assertEqual(1, len(approved_once.proposal.approvals))
        self.assert_model_error_unchanged(
            "DuplicateApproval",
            approved_once,
            lambda: group_approve(scope, state_account, approved_once, witnesses[0], members[0]),
        )

        approved_twice = group_approve(scope, state_account, approved_once, witnesses[1], members[1])
        self.assertEqual(2, len(approved_twice.proposal.approvals))
        self.assert_model_error_unchanged(
            "ThresholdAlreadyMet",
            approved_twice,
            lambda: group_approve(scope, state_account, approved_twice, witnesses[2], members[2]),
        )

        executed = group_execute(state_account, approved_twice)
        self.assertEqual(42, executed.value)
        self.assertTrue(executed.proposal.executed)
        self.assert_model_error_unchanged(
            "AlreadyExecuted",
            executed,
            lambda: group_execute(state_account, executed),
        )
        self.assert_model_error_unchanged(
            "AlreadyExecuted",
            executed,
            lambda: group_approve(scope, state_account, executed, witnesses[2], members[2]),
        )

        second = group_propose(scope, state_account, executed, witnesses[2], members[2], next_value=100)
        self.assertEqual(2, second.sequence)
        first_round = app_nullifier(scope, b"threshold", 1, witnesses[0].leaf.account_id, witnesses[0].secret)
        second_round = app_nullifier(scope, b"threshold", 2, witnesses[0].leaf.account_id, witnesses[0].secret)
        self.assertNotEqual(first_round, second_round)

    def test_group_public_state_does_not_contain_plain_witness_material(self):
        scope, root, witnesses, members = make_fixture(3)
        state_account = Account(STATE_ID, owner=PROGRAM_ID)
        state = group_create(root, member_count=3, threshold=2, initial_value=7)
        state = group_propose(scope, state_account, state, witnesses[0], members[0], next_value=42)
        state = group_approve(scope, state_account, state, witnesses[0], members[0])
        public_bytes = group_state_bytes(state)

        self.assertIn(root, public_bytes)
        self.assertIn(state.proposal.approvals[0], public_bytes)
        self.assertNotIn(witnesses[0].secret, public_bytes)
        self.assertNotIn(witnesses[0].viewing_public_key, public_bytes)
        self.assertNotIn(witnesses[0].leaf.account_id, public_bytes)
        self.assertNotIn(witnesses[0].leaf.salt, public_bytes)

    def test_threshold_bounds(self):
        _, root, _, _ = make_fixture(3)
        with self.assertRaises(ModelError) as zero:
            group_create(root, member_count=3, threshold=0, initial_value=7)
        self.assertEqual("InvalidThreshold", zero.exception.code)
        with self.assertRaises(ModelError) as oversized:
            group_create(root, member_count=3, threshold=4, initial_value=7)
        self.assertEqual("InvalidThreshold", oversized.exception.code)


if __name__ == "__main__":
    unittest.main()
