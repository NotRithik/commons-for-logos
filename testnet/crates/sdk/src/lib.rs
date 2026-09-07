//! Small host-side SDK for constructing v0.2.4 Astra program instructions.
//!
//! The LEZ v0.2.4 wallet serializes instructions with `risc0_zkvm::serde::to_vec`.
//! These helpers keep instruction construction typed while sharing the exact
//! instruction/state definitions with the guest programs.

pub use astra_logos_testnet_primitives as primitives;
use astra_logos_testnet_primitives::{
    DistributionInstruction, GroupInstruction, Hash32, MemberWitness,
};
use lee_core::program::InstructionData;
use serde::Serialize;

#[must_use]
pub const fn allowlist_create(root: Hash32, member_count: u32) -> DistributionInstruction {
    DistributionInstruction::Create { root, member_count }
}

#[must_use]
pub fn allowlist_claim(witness: MemberWitness) -> DistributionInstruction {
    DistributionInstruction::Claim { witness }
}

#[must_use]
pub const fn threshold_create(
    root: Hash32,
    member_count: u32,
    threshold: u32,
    initial_value: i64,
) -> GroupInstruction {
    GroupInstruction::Create {
        root,
        member_count,
        threshold,
        initial_value,
    }
}

#[must_use]
pub fn threshold_propose(witness: MemberWitness, next_value: i64) -> GroupInstruction {
    GroupInstruction::Propose {
        witness,
        next_value,
    }
}

#[must_use]
pub fn threshold_approve(witness: MemberWitness) -> GroupInstruction {
    GroupInstruction::Approve { witness }
}

#[must_use]
pub const fn threshold_execute() -> GroupInstruction {
    GroupInstruction::Execute
}

pub fn serialize_instruction<T: Serialize>(instruction: &T) -> Result<InstructionData, String> {
    risc0_zkvm::serde::to_vec(instruction).map_err(|err| err.to_string())
}

pub fn allowlist_create_data(root: Hash32, member_count: u32) -> Result<InstructionData, String> {
    serialize_instruction(&allowlist_create(root, member_count))
}

pub fn allowlist_claim_data(witness: MemberWitness) -> Result<InstructionData, String> {
    serialize_instruction(&allowlist_claim(witness))
}

pub fn threshold_create_data(
    root: Hash32,
    member_count: u32,
    threshold: u32,
    initial_value: i64,
) -> Result<InstructionData, String> {
    serialize_instruction(&threshold_create(
        root,
        member_count,
        threshold,
        initial_value,
    ))
}

pub fn threshold_propose_data(
    witness: MemberWitness,
    next_value: i64,
) -> Result<InstructionData, String> {
    serialize_instruction(&threshold_propose(witness, next_value))
}

pub fn threshold_approve_data(witness: MemberWitness) -> Result<InstructionData, String> {
    serialize_instruction(&threshold_approve(witness))
}

pub fn threshold_execute_data() -> Result<InstructionData, String> {
    serialize_instruction(&threshold_execute())
}
