//! Execute the actually published guest with legacy and replay-distinct inputs.
//! Synthetic public state only; no wallet, network, transaction submission or proof.
#[path = "../../../cli/src/execution_intent.rs"]
mod execution_intent;
use anyhow::{Context, Result, ensure};
use borsh::BorshDeserialize;
use commons_logos_testnet_primitives::{Group, GroupInstruction, Proposal};
use execution_intent::ExecutionIntent;
use lee::{
    program::Program,
    public_transaction::{Message, PublicTransaction, WitnessSet},
};
use lee_core::{
    account::{Account, AccountId, AccountWithMetadata},
    program::{ProgramId, ProgramOutput, validate_execution},
};
use risc0_zkvm::{ExecutorEnv, default_executor};
use serde_json::json;
use std::{fs, path::PathBuf};

fn fixture(
    program: ProgramId,
    sequence: u64,
    next_value: i64,
    approvals: usize,
) -> Result<AccountWithMetadata> {
    let group = Group {
        magic: *b"COMNSM01",
        root: [77; 32],
        member_count: 3,
        threshold: 2,
        value: next_value - 1,
        sequence,
        proposal: Some(Proposal {
            sequence,
            next_value,
            approvals: (0..approvals).map(|i| [i as u8 + 1; 32]).collect(),
            executed: false,
        }),
    };
    Ok(AccountWithMetadata {
        account: Account {
            program_owner: program,
            data: borsh::to_vec(&group)?.try_into()?,
            ..Account::default()
        },
        account_id: AccountId::new([42; 32]),
        is_authorized: false,
    })
}
fn execute(
    program: &Program,
    account: &AccountWithMetadata,
    words: &[u32],
) -> Result<(Group, u64)> {
    let accounts = vec![account.clone()];
    let mut env = ExecutorEnv::builder();
    env.session_limit(Some(32 * 1024 * 1024));
    env.write(&program.id())?
        .write(&Option::<ProgramId>::None)?
        .write(&accounts)?
        .write(&words.to_vec())?;
    let session = default_executor().execute(env.build()?, program.elf())?;
    let output: ProgramOutput = session.journal.decode()?;
    ensure!(
        output.self_program_id == program.id(),
        "guest identity mismatch"
    );
    ensure!(
        output.pre_states == accounts,
        "guest account input mismatch"
    );
    ensure!(
        output.instruction_data == words,
        "complete transport words were not committed"
    );
    validate_execution(&accounts, &output.post_states, program.id())?;
    let group = Group::try_from_slice(&output.post_states[0].account().data)?;
    Ok((group, session.cycles()))
}
fn hash(program: ProgramId, account: AccountId, words: Vec<u32>) -> [u8; 32] {
    PublicTransaction::new(
        Message::new_preserialized(program, vec![account], vec![], words),
        WitnessSet::from_raw_parts(vec![]),
    )
    .hash()
}
fn main() -> Result<()> {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .context("usage: check_execute_transport <published-threshold-program>")?,
    );
    let program = Program::new(fs::read(path)?.into())?;
    let mut report = Vec::new();
    let mut previous_hash = None;
    let legacy = Program::serialize_instruction(GroupInstruction::Execute)?;
    for (sequence, value) in [(1, 42), (2, 43), (3, 43)] {
        let account = fixture(program.id(), sequence, value, 2)?;
        let intent = ExecutionIntent::from_group(&Group::try_from_slice(&account.account.data)?)?;
        let words = intent.instruction_words()?;
        let (result, cycles) = execute(&program, &account, &words)?;
        intent.verify_postcondition(&result)?;
        let identity = hash(program.id(), account.account_id, words.clone());
        ensure!(
            Some(identity) != previous_hash,
            "new proposal reused previous transaction identity"
        );
        ensure!(
            identity
                == hash(
                    program.id(),
                    account.account_id,
                    intent.instruction_words()?
                ),
            "retry changed identity"
        );
        ensure!(
            identity != hash(program.id(), account.account_id, legacy.clone()),
            "tagged execution aliases legacy execution"
        );
        previous_hash = Some(identity);
        report.push(json!({"sequence":sequence,"value":value,"guest_cycles":cycles,"unsigned_transaction_hash":hex::encode(identity),"executed":result.proposal.as_ref().is_some_and(|p|p.executed),"transport_words":words.len()}));
    }
    let approved = fixture(program.id(), 2, 43, 2)?;
    let (legacy_result, _) = execute(&program, &approved, &legacy)?;
    ExecutionIntent {
        sequence: 2,
        next_value: 43,
    }
    .verify_postcondition(&legacy_result)?;
    let insufficient = fixture(program.id(), 2, 43, 1)?;
    ensure!(
        execute(
            &program,
            &insufficient,
            &ExecutionIntent {
                sequence: 2,
                next_value: 43
            }
            .instruction_words()?
        )
        .is_err(),
        "tag bypassed threshold check"
    );
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"passed":true,"program_id":program.id(),"real_published_guest_executed":true,"proof_generated":false,"network_transaction":false,"fixtures":"synthetic public accounts, no member secrets", "old_decoder_compatible":true,"full_instruction_words_committed":true,"threshold_failure_still_rejected":true,"sequence_tag_is_transport_not_consensus_guard":true,"runs":report})
        )?
    );
    Ok(())
}
