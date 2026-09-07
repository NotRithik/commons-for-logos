//! LEZ v0.2.4-compatible private allowlist and threshold primitives.
//!
//! This is a standalone port of the original private programs. The root
//! implementation remains unchanged; this crate uses the older v0.2.4 program
//! ABI: `ProgramId`, `ProgramInput`, `AccountPostState`, and `Claim`.
//!
//! Witnesses are sensitive and MUST be sent through LEZ private proving, never
//! serialized into a public transaction or a log.

use std::{fmt, io};

use borsh::{BorshDeserialize, BorshSerialize};
use lee_core::{
    Identifier, NullifierPublicKey,
    account::{Account, AccountId, AccountWithMetadata, Data},
    encryption::ViewingPublicKey,
    program::{AccountPostState, Claim, ProgramId},
};
use risc0_zkvm::sha::{Impl, Sha256 as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use sha2::{Digest, Sha256};

pub type Hash32 = [u8; 32];
pub const MAX_MEMBERS: usize = 256;
pub const MAX_TREE_DEPTH: usize = 8;
const DISTRIBUTION_MAGIC: [u8; 8] = *b"ASTRAD01";
const GROUP_MAGIC: [u8; 8] = *b"ASTRAM01";

const PRIVATE_ACCOUNT_ID_PREFIX: &[u8; 32] = b"/LEE/v0.3/AccountId/Private/\x00\x00\x00\x00";

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AccountCount = 1001,
    Unauthorized = 1002,
    AlreadyInitialized = 1003,
    WrongOwner = 1004,
    InvalidState = 1005,
    InvalidSize = 1006,
    IdentityMismatch = 1007,
    InvalidMembership = 1008,
    DuplicateClaim = 1009,
    CapacityReached = 1010,
    InvalidThreshold = 1011,
    PendingProposal = 1012,
    NoProposal = 1013,
    DuplicateApproval = 1014,
    ThresholdNotMet = 1015,
    AlreadyExecuted = 1016,
    ThresholdAlreadyMet = 1017,
    SequenceOverflow = 1018,
    DataTooLarge = 1019,
    InvalidViewingPublicKey = 1020,
    UninitializedMember = 1021,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ASTRA_ERROR_{}:{self:?}", *self as u32)
    }
}

impl std::error::Error for Error {}

/// Opaque ML-KEM-768 viewing-public-key bytes.
///
/// LEZ v0.2.4 exposes `ViewingPublicKey` as an ML-KEM encapsulation key. This
/// wrapper intentionally serializes raw bytes instead of relying on any upstream
/// Borsh implementation detail. It also lets the guest validate identity binding
/// without constructing a `ViewingPublicKey`, whose `from_bytes` constructor is
/// host-only in the checked-out v0.2.4 sources.
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct ViewingPublicKeyBytes {
    bytes: Vec<u8>,
}

impl ViewingPublicKeyBytes {
    pub const LEN: usize = ViewingPublicKey::LEN;

    pub fn new(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() != Self::LEN {
            return Err(Error::InvalidViewingPublicKey);
        }
        Ok(Self { bytes })
    }

    #[must_use]
    pub fn from_viewing_public_key(value: &ViewingPublicKey) -> Self {
        Self {
            bytes: value.to_bytes().to_vec(),
        }
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    #[cfg(feature = "host")]
    pub fn try_to_viewing_public_key(
        &self,
    ) -> Result<ViewingPublicKey, lee_core::error::LeeCoreError> {
        ViewingPublicKey::from_bytes(self.bytes.clone())
    }
}

impl fmt::Debug for ViewingPublicKeyBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ViewingPublicKeyBytes")
            .field("len", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl Serialize for ViewingPublicKeyBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Serialize::serialize(&self.bytes, serializer)
    }
}

impl<'de> Deserialize<'de> for ViewingPublicKeyBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = <Vec<u8> as Deserialize>::deserialize(deserializer)?;
        Self::new(bytes).map_err(de::Error::custom)
    }
}

impl BorshSerialize for ViewingPublicKeyBytes {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        BorshSerialize::serialize(&self.bytes, writer)
    }
}

impl BorshDeserialize for ViewingPublicKeyBytes {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let bytes = Vec::<u8>::deserialize_reader(reader)?;
        Self::new(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }
}

/// Hash each part with an explicit length to make the encoding unambiguous.
/// Domains distinguish tree leaves, nodes, contexts and claim nullifiers.
pub fn hash_parts(domain: &[u8], parts: &[&[u8]]) -> Hash32 {
    let mut hash = Sha256::new();
    hash.update((domain.len() as u64).to_le_bytes());
    hash.update(domain);
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}

#[must_use]
pub fn program_id_bytes(program: ProgramId) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (index, word) in program.iter().enumerate() {
        bytes[index * 4..(index + 1) * 4].copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

pub fn context(program: ProgramId, state: AccountId) -> Hash32 {
    let program = program_id_bytes(program);
    hash_parts(b"astra/context/v1", &[&program, state.as_ref()])
}

pub fn regular_private_account_id(
    npk: &NullifierPublicKey,
    vpk: &ViewingPublicKeyBytes,
    identifier: Identifier,
) -> Result<AccountId, Error> {
    if vpk.as_bytes().len() != ViewingPublicKeyBytes::LEN {
        return Err(Error::InvalidViewingPublicKey);
    }

    let mut bytes = [0_u8; 32 + 32 + ViewingPublicKeyBytes::LEN + 16];
    bytes[0..32].copy_from_slice(PRIVATE_ACCOUNT_ID_PREFIX);
    bytes[32..64].copy_from_slice(&npk.0);
    bytes[64..64 + ViewingPublicKeyBytes::LEN].copy_from_slice(vpk.as_bytes());
    bytes[64 + ViewingPublicKeyBytes::LEN..].copy_from_slice(&identifier.to_le_bytes());

    Ok(AccountId::new(
        Impl::hash_bytes(&bytes)
            .as_bytes()
            .try_into()
            .expect("RISC0 SHA-256 output is 32 bytes"),
    ))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct MemberLeaf {
    pub account_id: AccountId,
    pub salt: Hash32,
    pub entitlement: u64,
}

impl MemberLeaf {
    pub fn commitment(&self, scope: &Hash32) -> Hash32 {
        hash_parts(
            b"astra/member/v1",
            &[
                scope,
                self.account_id.as_ref(),
                &self.salt,
                &self.entitlement.to_le_bytes(),
            ],
        )
    }
}

/// Intentionally does not implement Debug: it carries a secret witness.
#[derive(Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct MemberWitness {
    pub leaf: MemberLeaf,
    pub nullifier_secret_key: Hash32,
    pub viewing_public_key: ViewingPublicKeyBytes,
    pub identifier: Identifier,
    pub leaf_index: u32,
    pub siblings: Vec<Hash32>,
}

impl MemberWitness {
    fn verify(
        &self,
        member: &AccountWithMetadata,
        root: &Hash32,
        members: u32,
        scope: &Hash32,
    ) -> Result<(), Error> {
        if !member.is_authorized {
            return Err(Error::Unauthorized);
        }
        // v0.2.4 rejects a default-owner account after its first private nonce
        // rotation. Fresh accounts are explicitly claimed in output(); malformed
        // legacy accounts must not be accepted and turned into unusable members.
        if member.account.program_owner == lee_core::program::DEFAULT_PROGRAM_ID
            && member.account != Account::default()
        {
            return Err(Error::UninitializedMember);
        }
        let npk = NullifierPublicKey::from(&self.nullifier_secret_key);
        let derived = regular_private_account_id(&npk, &self.viewing_public_key, self.identifier)?;
        // Bind nullifier uniqueness to the ACTUAL authorized account, not to a
        // caller-chosen secret. Changing the witness secret cannot buy a second claim.
        if derived != member.account_id || self.leaf.account_id != member.account_id {
            return Err(Error::IdentityMismatch);
        }
        if self.leaf.entitlement == 0 {
            return Err(Error::InvalidMembership);
        }
        verify_membership(
            root,
            self.leaf.commitment(scope),
            members,
            self.leaf_index,
            &self.siblings,
        )
    }

    fn nullifier(&self, scope: &Hash32, purpose: &[u8], sequence: u64) -> Hash32 {
        hash_parts(
            b"astra/nullifier/v1",
            &[
                scope,
                purpose,
                &sequence.to_le_bytes(),
                self.leaf.account_id.as_ref(),
                &self.nullifier_secret_key,
            ],
        )
    }
}

fn check_size(count: u32) -> Result<usize, Error> {
    let count = count as usize;
    if count == 0 || count > MAX_MEMBERS {
        return Err(Error::InvalidSize);
    }
    Ok(count)
}

fn node(left: &Hash32, right: &Hash32) -> Hash32 {
    hash_parts(b"astra/node/v1", &[left, right])
}

/// Canonical padded binary tree. Empty leaves cannot be claimed because the
/// proof additionally checks that the index is below the original leaf count.
pub fn merkle_tree(leaves: &[Hash32]) -> Result<Vec<Vec<Hash32>>, Error> {
    check_size(leaves.len().try_into().map_err(|_| Error::InvalidSize)?)?;
    let mut bottom = leaves.to_vec();
    bottom.resize(
        leaves.len().next_power_of_two(),
        hash_parts(b"astra/empty/v1", &[]),
    );
    let mut levels = vec![bottom];
    while levels.last().unwrap().len() > 1 {
        let next = levels
            .last()
            .unwrap()
            .chunks_exact(2)
            .map(|pair| node(&pair[0], &pair[1]))
            .collect();
        levels.push(next);
    }
    Ok(levels)
}

pub fn merkle_proof(leaves: &[Hash32], index: usize) -> Result<(Hash32, Vec<Hash32>), Error> {
    if index >= leaves.len() {
        return Err(Error::InvalidMembership);
    }
    let tree = merkle_tree(leaves)?;
    let mut cursor = index;
    let mut proof = Vec::new();
    for level in tree.iter().take(tree.len() - 1) {
        proof.push(level[cursor ^ 1]);
        cursor /= 2;
    }
    Ok((tree.last().unwrap()[0], proof))
}

fn verify_membership(
    root: &Hash32,
    leaf: Hash32,
    count: u32,
    index: u32,
    proof: &[Hash32],
) -> Result<(), Error> {
    let count = check_size(count)?;
    let depth = count.next_power_of_two().trailing_zeros() as usize;
    if index as usize >= count || proof.len() != depth || proof.len() > MAX_TREE_DEPTH {
        return Err(Error::InvalidMembership);
    }
    let mut current = leaf;
    let mut cursor = index;
    for sibling in proof {
        current = if cursor & 1 == 0 {
            node(&current, sibling)
        } else {
            node(sibling, &current)
        };
        cursor >>= 1;
    }
    if current != *root {
        return Err(Error::InvalidMembership);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Distribution {
    pub magic: [u8; 8],
    pub root: Hash32,
    pub member_count: u32,
    pub claims: Vec<Hash32>,
}

#[derive(Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum DistributionInstruction {
    Create { root: Hash32, member_count: u32 },
    Claim { witness: MemberWitness },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Proposal {
    pub sequence: u64,
    pub next_value: i64,
    pub approvals: Vec<Hash32>,
    pub executed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Group {
    pub magic: [u8; 8],
    pub root: Hash32,
    pub member_count: u32,
    pub threshold: u32,
    pub value: i64,
    pub sequence: u64,
    pub proposal: Option<Proposal>,
}

#[derive(Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum GroupInstruction {
    Create {
        root: Hash32,
        member_count: u32,
        threshold: u32,
        initial_value: i64,
    },
    Propose {
        witness: MemberWitness,
        next_value: i64,
    },
    Approve {
        witness: MemberWitness,
    },
    Execute,
}

fn check_new(account: &AccountWithMetadata) -> Result<(), Error> {
    if !account.is_authorized {
        return Err(Error::Unauthorized);
    }
    // LEZ v0.2.4 only permits initializing data on a truly default account via
    // AccountPostState::new_claimed(..., Claim::Authorized). The program output
    // itself must not modify program_owner or nonce.
    if account.account != Account::default() {
        return Err(Error::AlreadyInitialized);
    }
    Ok(())
}

fn check_owned(account: &AccountWithMetadata, program: ProgramId) -> Result<(), Error> {
    if account.account.program_owner != program {
        return Err(Error::WrongOwner);
    }
    Ok(())
}

fn encode<T: BorshSerialize>(value: &T) -> Result<Data, Error> {
    borsh::to_vec(value)
        .map_err(|_| Error::InvalidState)?
        .try_into()
        .map_err(|_| Error::DataTooLarge)
}

fn output(
    accounts: &[AccountWithMetadata],
    state_data: Data,
    claim_state_account: bool,
) -> Vec<AccountPostState> {
    accounts
        .iter()
        .enumerate()
        .map(|(index, account)| {
            let mut post = account.account.clone();
            if index == 0 {
                post.data = state_data.clone();
                if claim_state_account {
                    return AccountPostState::new_claimed(post, Claim::Authorized);
                }
            }
            // Every private use rotates its nonce, even when balance/data are
            // unchanged. Claim a brand-new, authorized zero-value member now so
            // later proposals/approvals can reuse that identity. Existing member
            // ownership, data and balance are never changed by this program.
            if index > 0 && post == Account::default() {
                return AccountPostState::new_claimed(post, Claim::Authorized);
            }
            AccountPostState::new(post)
        })
        .collect()
}

pub fn execute_distribution(
    program: ProgramId,
    accounts: &[AccountWithMetadata],
    instruction: DistributionInstruction,
) -> Result<Vec<AccountPostState>, Error> {
    let expected = if matches!(&instruction, DistributionInstruction::Create { .. }) {
        1
    } else {
        2
    };
    if accounts.len() != expected {
        return Err(Error::AccountCount);
    }
    let state_account = &accounts[0];
    let (next, claim_state_account) = match instruction {
        DistributionInstruction::Create { root, member_count } => {
            check_new(state_account)?;
            check_size(member_count)?;
            (
                Distribution {
                    magic: DISTRIBUTION_MAGIC,
                    root,
                    member_count,
                    claims: Vec::new(),
                },
                true,
            )
        }
        DistributionInstruction::Claim { witness } => {
            check_owned(state_account, program)?;
            let mut state = Distribution::try_from_slice(&state_account.account.data)
                .map_err(|_| Error::InvalidState)?;
            if state.magic != DISTRIBUTION_MAGIC {
                return Err(Error::InvalidState);
            }
            check_size(state.member_count)?;
            let scope = context(program, state_account.account_id);
            witness.verify(&accounts[1], &state.root, state.member_count, &scope)?;
            let nullifier = witness.nullifier(&scope, b"allowlist", 0);
            if state.claims.contains(&nullifier) {
                return Err(Error::DuplicateClaim);
            }
            if state.claims.len() >= state.member_count as usize {
                return Err(Error::CapacityReached);
            }
            state.claims.push(nullifier);
            (state, false)
        }
    };
    // No mutation takes place until every check succeeds. A failed claim has no
    // state diff and can be retried against a fresh sequencer snapshot.
    Ok(output(accounts, encode(&next)?, claim_state_account))
}

pub fn execute_group(
    program: ProgramId,
    accounts: &[AccountWithMetadata],
    instruction: GroupInstruction,
) -> Result<Vec<AccountPostState>, Error> {
    let expected = match &instruction {
        GroupInstruction::Create { .. } | GroupInstruction::Execute => 1,
        _ => 2,
    };
    if accounts.len() != expected {
        return Err(Error::AccountCount);
    }
    let state_account = &accounts[0];
    if let GroupInstruction::Create {
        root,
        member_count,
        threshold,
        initial_value,
    } = instruction
    {
        check_new(state_account)?;
        check_size(member_count)?;
        if threshold == 0 || threshold > member_count {
            return Err(Error::InvalidThreshold);
        }
        let state = Group {
            magic: GROUP_MAGIC,
            root,
            member_count,
            threshold,
            value: initial_value,
            sequence: 0,
            proposal: None,
        };
        return Ok(output(accounts, encode(&state)?, true));
    }
    check_owned(state_account, program)?;
    let mut state =
        Group::try_from_slice(&state_account.account.data).map_err(|_| Error::InvalidState)?;
    if state.magic != GROUP_MAGIC {
        return Err(Error::InvalidState);
    }
    check_size(state.member_count)?;
    if state.threshold == 0 || state.threshold > state.member_count {
        return Err(Error::InvalidThreshold);
    }
    let scope = context(program, state_account.account_id);
    match instruction {
        GroupInstruction::Propose {
            witness,
            next_value,
        } => {
            witness.verify(&accounts[1], &state.root, state.member_count, &scope)?;
            if state.proposal.as_ref().is_some_and(|p| !p.executed) {
                return Err(Error::PendingProposal);
            }
            state.sequence = state
                .sequence
                .checked_add(1)
                .ok_or(Error::SequenceOverflow)?;
            state.proposal = Some(Proposal {
                sequence: state.sequence,
                next_value,
                approvals: Vec::new(),
                executed: false,
            });
        }
        GroupInstruction::Approve { witness } => {
            witness.verify(&accounts[1], &state.root, state.member_count, &scope)?;
            let proposal = state.proposal.as_mut().ok_or(Error::NoProposal)?;
            if proposal.executed {
                return Err(Error::AlreadyExecuted);
            }
            let nullifier = witness.nullifier(&scope, b"threshold", proposal.sequence);
            if proposal.approvals.contains(&nullifier) {
                return Err(Error::DuplicateApproval);
            }
            if proposal.approvals.len() >= state.threshold as usize {
                return Err(Error::ThresholdAlreadyMet);
            }
            proposal.approvals.push(nullifier);
        }
        GroupInstruction::Execute => {
            let proposal = state.proposal.as_mut().ok_or(Error::NoProposal)?;
            if proposal.executed {
                return Err(Error::AlreadyExecuted);
            }
            if proposal.approvals.len() < state.threshold as usize {
                return Err(Error::ThresholdNotMet);
            }
            state.value = proposal.next_value;
            proposal.executed = true;
        }
        GroupInstruction::Create { .. } => unreachable!(),
    }
    Ok(output(accounts, encode(&state)?, false))
}
