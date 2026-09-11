//! Replay-distinct transport for the deployed v0.2.4 permissionless executor.
//!
//! The deployed guest reads one `GroupInstruction` from a word slice and commits
//! the complete instruction slice. A typed tuple preserves that instruction as
//! its prefix and adds a deterministic, public proposal tag. This is transport
//! deduplication, NOT a new consensus-level expected-sequence check: the legacy
//! guest still executes whichever proposal is currently threshold-approved.
//! Never report success without checking the exact requested postcondition.
use anyhow::{Context, Result, ensure};
use commons_logos_testnet_primitives::{Group, GroupInstruction};
use lee::program::Program;
use lee_core::program::InstructionData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionIntent {
    pub sequence: u64,
    pub next_value: i64,
}
impl ExecutionIntent {
    pub fn from_group(group: &Group) -> Result<Self> {
        ensure!(group.magic == *b"COMNSM01", "invalid threshold state magic");
        let proposal = group.proposal.as_ref().context("no current proposal")?;
        ensure!(
            proposal.sequence == group.sequence,
            "proposal sequence differs from group"
        );
        ensure!(!proposal.executed, "proposal is already executed");
        ensure!(
            group.threshold > 0 && group.threshold <= group.member_count,
            "invalid threshold"
        );
        ensure!(
            proposal.approvals.len() >= group.threshold as usize,
            "threshold not met"
        );
        Ok(Self {
            sequence: proposal.sequence,
            next_value: proposal.next_value,
        })
    }
    pub fn instruction_words(self) -> Result<InstructionData> {
        // Serde tuple encoding adds no tuple header. Verify against the actual
        // published guest in integration/src/bin/check_execute_transport.rs.
        Ok(Program::serialize_instruction((
            GroupInstruction::Execute,
            *b"COMEXE01",
            self.sequence,
            self.next_value,
        ))?)
    }
    pub fn verify_postcondition(self, group: &Group) -> Result<()> {
        ensure!(group.magic == *b"COMNSM01", "wrong returned state type");
        let proposal = group
            .proposal
            .as_ref()
            .context("returned proposal missing")?;
        ensure!(
            group.sequence == self.sequence && proposal.sequence == self.sequence,
            "current state no longer identifies the requested proposal"
        );
        ensure!(
            proposal.executed
                && proposal.next_value == self.next_value
                && group.value == self.next_value,
            "confirmed transaction did not execute the requested value"
        );
        ensure!(
            group.threshold > 0
                && group.threshold <= group.member_count
                && proposal.approvals.len() >= group.threshold as usize,
            "returned threshold not met"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commons_logos_testnet_primitives::Proposal;
    use lee::public_transaction::{Message, PublicTransaction, WitnessSet};
    use lee_core::account::AccountId;
    fn group() -> Group {
        Group {
            magic: *b"COMNSM01",
            root: [7; 32],
            member_count: 3,
            threshold: 2,
            value: 42,
            sequence: 2,
            proposal: Some(Proposal {
                sequence: 2,
                next_value: 43,
                approvals: vec![[1; 32], [2; 32]],
                executed: false,
            }),
        }
    }
    fn hash(intent: ExecutionIntent) -> [u8; 32] {
        let message = Message::new_preserialized(
            [1; 8],
            vec![AccountId::new([2; 32])],
            vec![],
            intent.instruction_words().unwrap(),
        );
        PublicTransaction::new(message, WitnessSet::from_raw_parts(vec![])).hash()
    }
    #[test]
    fn tuple_preserves_deployed_instruction_prefix() {
        let prefix = Program::serialize_instruction(GroupInstruction::Execute).unwrap();
        let words = ExecutionIntent::from_group(&group())
            .unwrap()
            .instruction_words()
            .unwrap();
        assert_eq!(&words[..prefix.len()], prefix.as_slice());
        assert!(words.len() > prefix.len());
    }
    #[test]
    fn retries_have_the_same_hash_and_different_proposals_do_not() {
        let a = ExecutionIntent {
            sequence: 1,
            next_value: 42,
        };
        assert_eq!(hash(a), hash(a));
        assert_ne!(
            hash(a),
            hash(ExecutionIntent {
                sequence: 2,
                next_value: 43
            })
        );
        assert_ne!(
            hash(a),
            hash(ExecutionIntent {
                sequence: 1,
                next_value: 43
            })
        );
    }
    #[test]
    fn old_receipt_pending_state_is_not_success() {
        let g = group();
        let intent = ExecutionIntent::from_group(&g).unwrap();
        assert!(intent.verify_postcondition(&g).is_err());
    }
    #[test]
    fn exact_completed_proposal_is_success() {
        let mut g = group();
        let intent = ExecutionIntent::from_group(&g).unwrap();
        g.value = 43;
        g.proposal.as_mut().unwrap().executed = true;
        assert!(intent.verify_postcondition(&g).is_ok());
    }
    #[test]
    fn wrong_value_sequence_or_incomplete_threshold_are_rejected() {
        let g = group();
        let intent = ExecutionIntent::from_group(&g).unwrap();
        for mode in 0..4 {
            let mut other = g.clone();
            other.value = 43;
            other.proposal.as_mut().unwrap().executed = true;
            match mode {
                0 => other.value = 42,
                1 => other.sequence = 3,
                2 => other.proposal.as_mut().unwrap().sequence = 3,
                _ => other.proposal.as_mut().unwrap().approvals.clear(),
            }
            assert!(intent.verify_postcondition(&other).is_err());
        }
    }
    #[test]
    fn partial_and_already_executed_inputs_are_not_prepared() {
        let mut g = group();
        g.proposal.as_mut().unwrap().approvals.pop();
        assert!(ExecutionIntent::from_group(&g).is_err());
        let mut g = group();
        g.proposal.as_mut().unwrap().executed = true;
        assert!(ExecutionIntent::from_group(&g).is_err());
    }
}
