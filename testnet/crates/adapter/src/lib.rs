//! Guest-compatible consumption of an executed Commons threshold decision.
//!
//! These checks must run inside the consuming LEZ program against authenticated
//! account inputs. Parsing caller-supplied JSON off chain does not confer authority.
//! The policy binding must come from trusted constants or protected consumer state,
//! not from the untrusted caller's choice on each invocation.
#![forbid(unsafe_code)]

use borsh::{BorshDeserialize, BorshSerialize};
use commons_logos_testnet_primitives::{Group, MAX_MEMBERS};
use lee_core::{
    account::{Account, AccountId, AccountWithMetadata},
    program::{AccountPostState, Claim, ProgramId},
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The existing, independently deployed Commons threshold program. This adapter
/// does not change its image or require members to recreate their policy.
pub const THRESHOLD_PROGRAM_ID: ProgramId = [
    3889587528, 4072157194, 1675466154, 3386074792, 1392369498, 2156404430, 3280262519, 3599087292,
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct PolicyBinding {
    pub program_id: ProgramId,
    pub state_account: AccountId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutedDecision {
    pub sequence: u64,
    pub value: i64,
    pub threshold: u32,
    pub members: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AdapterError {
    AccountCount = 2001,
    WrongPolicyAccount = 2002,
    WrongPolicyProgram = 2003,
    InvalidPolicyState = 2004,
    DecisionNotExecuted = 2005,
    DecisionAlreadyConsumed = 2006,
    UnexpectedDecision = 2007,
    ValueOutsideBounds = 2008,
    UnauthorizedInitialization = 2009,
    AlreadyInitialized = 2010,
    WrongConsumerProgram = 2011,
    InvalidConsumerState = 2012,
    DataTooLarge = 2013,
}
impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "COMMONS_ADAPTER_{}_{self:?}", *self as u32)
    }
}
impl std::error::Error for AdapterError {}

fn read_policy(
    policy: &AccountWithMetadata,
    binding: &PolicyBinding,
) -> Result<Group, AdapterError> {
    if policy.account_id != binding.state_account {
        return Err(AdapterError::WrongPolicyAccount);
    }
    if binding.program_id == [0; 8] || policy.account.program_owner != binding.program_id {
        return Err(AdapterError::WrongPolicyProgram);
    }
    let group = Group::try_from_slice(&policy.account.data)
        .map_err(|_| AdapterError::InvalidPolicyState)?;
    if group.magic != *b"COMNSM01"
        || group.member_count == 0
        || group.member_count as usize > MAX_MEMBERS
        || group.threshold == 0
        || group.threshold > group.member_count
    {
        return Err(AdapterError::InvalidPolicyState);
    }
    Ok(group)
}

/// Read the latest fully executed decision, without requesting any member key or
/// changing the foreign policy account. A pending or initial-only policy is not
/// an executed authorization. A newer pending proposal temporarily blocks this
/// helper even though the previous value remains in the policy's state.
pub fn read_executed_decision(
    policy: &AccountWithMetadata,
    binding: &PolicyBinding,
) -> Result<ExecutedDecision, AdapterError> {
    let group = read_policy(policy, binding)?;
    let proposal = group
        .proposal
        .as_ref()
        .ok_or(AdapterError::DecisionNotExecuted)?;
    if !proposal.executed {
        return Err(AdapterError::DecisionNotExecuted);
    }
    if group.sequence == 0
        || proposal.sequence != group.sequence
        || proposal.next_value != group.value
        || proposal.approvals.len() < group.threshold as usize
        || proposal.approvals.len() > group.member_count as usize
    {
        return Err(AdapterError::InvalidPolicyState);
    }
    for (index, nullifier) in proposal.approvals.iter().enumerate() {
        if proposal.approvals[..index].contains(nullifier) {
            return Err(AdapterError::InvalidPolicyState);
        }
    }
    Ok(ExecutedDecision {
        sequence: group.sequence,
        value: group.value,
        threshold: group.threshold,
        members: group.member_count,
    })
}

/// Enforce per-consumer replay protection. Persist the returned sequence only as
/// part of the same successful state transition as the application action.
pub fn consume_after(
    policy: &AccountWithMetadata,
    binding: &PolicyBinding,
    last_consumed_sequence: u64,
) -> Result<ExecutedDecision, AdapterError> {
    let decision = read_executed_decision(policy, binding)?;
    if decision.sequence <= last_consumed_sequence {
        return Err(AdapterError::DecisionAlreadyConsumed);
    }
    Ok(decision)
}

/// A deliberately small reference consumer. It owns a DIFFERENT state account
/// from the Commons policy and accepts only bounded setting changes authorized
/// by that pinned policy. It does not move tokens or implement a call router.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct GovernedSetting {
    pub magic: [u8; 8],
    pub policy: PolicyBinding,
    pub minimum: i64,
    pub maximum: i64,
    pub value: i64,
    pub last_consumed_sequence: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GovernedSettingInstruction {
    Initialize {
        policy: PolicyBinding,
        minimum: i64,
        maximum: i64,
        initial_value: i64,
    },
    Apply {
        expected_sequence: u64,
        expected_value: i64,
    },
}

/// Account order: [consumer state (owned here), threshold policy (read only)].
/// Initialization requires the fresh consumer account's authorization. Later
/// application is public and permissionless after the pinned policy executes.
pub fn execute_governed_setting(
    consumer_program: ProgramId,
    accounts: &[AccountWithMetadata],
    instruction: GovernedSettingInstruction,
) -> Result<Vec<AccountPostState>, AdapterError> {
    if accounts.len() != 2 || accounts[0].account_id == accounts[1].account_id {
        return Err(AdapterError::AccountCount);
    }
    let consumer = &accounts[0];
    let policy = &accounts[1];
    let initializing = matches!(&instruction, GovernedSettingInstruction::Initialize { .. });
    let state = match instruction {
        GovernedSettingInstruction::Initialize {
            policy: binding,
            minimum,
            maximum,
            initial_value,
        } => {
            if !consumer.is_authorized {
                return Err(AdapterError::UnauthorizedInitialization);
            }
            if consumer.account != Account::default() {
                return Err(AdapterError::AlreadyInitialized);
            }
            if binding.program_id != THRESHOLD_PROGRAM_ID {
                return Err(AdapterError::WrongPolicyProgram);
            }
            read_policy(policy, &binding)?;
            if minimum > maximum || initial_value < minimum || initial_value > maximum {
                return Err(AdapterError::ValueOutsideBounds);
            }
            GovernedSetting {
                magic: *b"COMNSC01",
                policy: binding,
                minimum,
                maximum,
                value: initial_value,
                last_consumed_sequence: 0,
            }
        }
        GovernedSettingInstruction::Apply {
            expected_sequence,
            expected_value,
        } => {
            if consumer.account.program_owner != consumer_program {
                return Err(AdapterError::WrongConsumerProgram);
            }
            let mut state = GovernedSetting::try_from_slice(&consumer.account.data)
                .map_err(|_| AdapterError::InvalidConsumerState)?;
            if state.magic != *b"COMNSC01"
                || state.policy.program_id != THRESHOLD_PROGRAM_ID
                || state.minimum > state.maximum
                || state.value < state.minimum
                || state.value > state.maximum
            {
                return Err(AdapterError::InvalidConsumerState);
            }
            let decision = consume_after(policy, &state.policy, state.last_consumed_sequence)?;
            if decision.sequence != expected_sequence || decision.value != expected_value {
                return Err(AdapterError::UnexpectedDecision);
            }
            if decision.value < state.minimum || decision.value > state.maximum {
                return Err(AdapterError::ValueOutsideBounds);
            }
            state.value = decision.value;
            state.last_consumed_sequence = decision.sequence;
            state
        }
    };
    let mut next = consumer.account.clone();
    next.data = borsh::to_vec(&state)
        .map_err(|_| AdapterError::InvalidConsumerState)?
        .try_into()
        .map_err(|_| AdapterError::DataTooLarge)?;
    let consumer_post = if initializing {
        AccountPostState::new_claimed(next, Claim::Authorized)
    } else {
        AccountPostState::new(next)
    };
    // No foreign-owner data, balance, owner or nonce mutation is requested.
    Ok(vec![
        consumer_post,
        AccountPostState::new(policy.account.clone()),
    ])
}
