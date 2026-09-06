//! Original reference programs for private LEZ allowlist registration and
//! private threshold-governed parameter updates. No tokens are moved by either
//! program. Witnesses are sensitive and MUST be sent through LEZ private proving,
//! never serialized into a public transaction or a log.

use borsh::{BorshDeserialize, BorshSerialize};
use lee_core::{
    NullifierPublicKey,
    account::{AccountId, AccountWithMetadata, BalanceDiff, Data},
    encryption::ViewingPublicKey,
    program::{AccountStateDiff, DEFAULT_PROGRAM_OWNER},
};
use sha2::{Digest, Sha256};

pub type Hash32 = [u8; 32];
pub const MAX_MEMBERS: usize = 256;
pub const MAX_TREE_DEPTH: usize = 8;
const DISTRIBUTION_MAGIC: [u8; 8] = *b"ASTRAD01";
const GROUP_MAGIC: [u8; 8] = *b"ASTRAM01";

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
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ASTRA_ERROR_{}:{self:?}", *self as u32)
    }
}
impl std::error::Error for Error {}

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

pub fn context(program: AccountId, state: AccountId) -> Hash32 {
    hash_parts(b"astra/context/v1", &[program.as_ref(), state.as_ref()])
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
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
#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub struct MemberWitness {
    pub leaf: MemberLeaf,
    pub nullifier_secret_key: Hash32,
    pub viewing_public_key: ViewingPublicKey,
    pub identifier: u128,
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
        let npk = NullifierPublicKey::from(&self.nullifier_secret_key);
        let derived =
            AccountId::for_regular_private_account(&npk, &self.viewing_public_key, self.identifier);
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

#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct Distribution {
    pub magic: [u8; 8],
    pub root: Hash32,
    pub member_count: u32,
    pub claims: Vec<Hash32>,
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub enum DistributionInstruction {
    Create { root: Hash32, member_count: u32 },
    Claim { witness: MemberWitness },
}

#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct Proposal {
    pub sequence: u64,
    pub next_value: i64,
    pub approvals: Vec<Hash32>,
    pub executed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct Group {
    pub magic: [u8; 8],
    pub root: Hash32,
    pub member_count: u32,
    pub threshold: u32,
    pub value: i64,
    pub sequence: u64,
    pub proposal: Option<Proposal>,
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
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
    if account.account.program_owner != DEFAULT_PROGRAM_OWNER
        || !account.account.data.is_empty()
        || account.account.nonce.0 != 0
    {
        return Err(Error::AlreadyInitialized);
    }
    Ok(())
}

fn check_owned(account: &AccountWithMetadata, program: AccountId) -> Result<(), Error> {
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

fn output(accounts: &[AccountWithMetadata], data: Data) -> Vec<AccountStateDiff> {
    accounts
        .iter()
        .enumerate()
        .map(|(index, account)| {
            AccountStateDiff::new(
                account.clone(),
                BalanceDiff::Add(0),
                if index == 0 {
                    data.clone()
                } else {
                    account.account.data.clone()
                },
            )
        })
        .collect()
}

pub fn execute_distribution(
    program: AccountId,
    accounts: &[AccountWithMetadata],
    instruction: DistributionInstruction,
) -> Result<Vec<AccountStateDiff>, Error> {
    let expected = if matches!(&instruction, DistributionInstruction::Create { .. }) {
        1
    } else {
        2
    };
    if accounts.len() != expected {
        return Err(Error::AccountCount);
    }
    let state_account = &accounts[0];
    let next = match instruction {
        DistributionInstruction::Create { root, member_count } => {
            check_new(state_account)?;
            check_size(member_count)?;
            Distribution {
                magic: DISTRIBUTION_MAGIC,
                root,
                member_count,
                claims: Vec::new(),
            }
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
            state
        }
    };
    // No mutation takes place until every check succeeds. A failed claim has no
    // state diff and can be retried against a fresh sequencer snapshot.
    Ok(output(accounts, encode(&next)?))
}

pub fn execute_group(
    program: AccountId,
    accounts: &[AccountWithMetadata],
    instruction: GroupInstruction,
) -> Result<Vec<AccountStateDiff>, Error> {
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
        return Ok(output(accounts, encode(&state)?));
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
    Ok(output(accounts, encode(&state)?))
}
