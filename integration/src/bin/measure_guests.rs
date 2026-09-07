//! Deterministic guest-cycle measurements. No wallet, network, or proof is used.
use anyhow::{Context, Result, ensure};
use borsh::BorshDeserialize;
use commons_logos_testnet_primitives::*;
use lee::program::Program;
use lee_core::{
    NullifierPublicKey,
    account::{Account, AccountId, AccountWithMetadata},
    encryption::ViewingPublicKey,
    program::{AccountPostState, ProgramId, ProgramOutput, validate_execution},
};
use risc0_zkvm::{ExecutorEnv, default_executor};
use serde::Serialize;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, time::Instant};

struct Fixture {
    state: AccountWithMetadata,
    members: Vec<AccountWithMetadata>,
    witnesses: Vec<MemberWitness>,
    root: Hash32,
}
impl Fixture {
    fn new(program: ProgramId, size: usize) -> Result<Self> {
        let state = AccountWithMetadata {
            account: Account::default(),
            is_authorized: true,
            account_id: AccountId::new([42; 32]),
        };
        let scope = context(program, state.account_id);
        let mut members = Vec::new();
        let mut witnesses = Vec::new();
        for index in 0..size {
            let secret = hash_parts(
                b"public-benchmark-fixture-not-a-wallet",
                &[&(index as u64).to_le_bytes()],
            );
            let viewing = ViewingPublicKey::from_seed(&secret, &[7; 32]);
            let viewing_public_key = ViewingPublicKeyBytes::from_viewing_public_key(&viewing);
            let account_id = regular_private_account_id(
                &NullifierPublicKey::from(&secret),
                &viewing_public_key,
                index as u128,
            )?;
            members.push(AccountWithMetadata {
                account: Account {
                    nonce: (17 + index as u128).into(),
                    program_owner: [99; 8],
                    data: vec![index as u8, 99].try_into()?,
                    ..Account::default()
                },
                is_authorized: true,
                account_id,
            });
            witnesses.push(MemberWitness {
                leaf: MemberLeaf {
                    account_id,
                    salt: hash_parts(b"public-benchmark-salt", &[&secret]),
                    entitlement: 1,
                },
                nullifier_secret_key: secret,
                viewing_public_key,
                identifier: index as u128,
                leaf_index: index as u32,
                siblings: Vec::new(),
            });
        }
        let leaves: Vec<_> = witnesses
            .iter()
            .map(|w| w.leaf.commitment(&scope))
            .collect();
        for (index, witness) in witnesses.iter_mut().enumerate() {
            witness.siblings = merkle_proof(&leaves, index)?.1;
        }
        Ok(Self {
            state,
            members,
            witnesses,
            root: merkle_proof(&leaves, 0)?.0,
        })
    }
    fn apply_state(&mut self, program: ProgramId, output: &[AccountPostState], creating: bool) {
        self.state.account = output[0].account().clone();
        if creating {
            self.state.account.program_owner = program;
        }
        self.state.account.nonce.0 += 1;
    }
}
fn run<T: Serialize>(
    program: &Program,
    label: &str,
    size: usize,
    accounts: &[AccountWithMetadata],
    ix: &T,
    results: &mut Vec<Value>,
) -> Result<Vec<AccountPostState>> {
    let words = Program::serialize_instruction(ix)?;
    let mut env = ExecutorEnv::builder();
    env.session_limit(Some(32 * 1024 * 1024));
    env.write(&program.id())?
        .write(&Option::<ProgramId>::None)?
        .write(&accounts)?
        .write(&words)?;
    let start = Instant::now();
    let session = default_executor().execute(env.build()?, program.elf())?;
    let output: ProgramOutput = session.journal.decode()?;
    ensure!(output.pre_states == accounts, "guest pre-state mismatch");
    validate_execution(accounts, &output.post_states, program.id())?;
    results.push(
        json!({"operation":label, "member_count":size, "guest_user_cycles":session.cycles(),
        "segments":session.segments.len(), "executor_wall_seconds":start.elapsed().as_secs_f64(),
        "program_image_id":program.id(), "proof_generated":false, "network_transaction":false}),
    );
    // Private journal contents are neither printed nor persisted.
    Ok(output.post_states)
}
fn main() -> Result<()> {
    let artifacts = PathBuf::from(
        std::env::args()
            .nth(1)
            .context("usage: measure_guests <packed-program-directory>")?,
    );
    let allow = Program::new(fs::read(artifacts.join("commons_allowlist"))?.into())?;
    let threshold = Program::new(fs::read(artifacts.join("commons_threshold"))?.into())?;
    let mut results = Vec::new();
    for size in [3, 10, 256] {
        let mut a = Fixture::new(allow.id(), size)?;
        let out = run(
            &allow,
            "allowlist.create",
            size,
            &[a.state.clone()],
            &DistributionInstruction::Create {
                root: a.root,
                member_count: size as u32,
            },
            &mut results,
        )?;
        a.apply_state(allow.id(), &out, true);
        let out = run(
            &allow,
            "allowlist.claim",
            size,
            &[a.state.clone(), a.members[0].clone()],
            &DistributionInstruction::Claim {
                witness: a.witnesses[0].clone(),
            },
            &mut results,
        )?;
        a.apply_state(allow.id(), &out, false);
        ensure!(
            Distribution::try_from_slice(&a.state.account.data)?
                .claims
                .len()
                == 1,
            "claim did not update state"
        );

        let mut g = Fixture::new(threshold.id(), size)?;
        let out = run(
            &threshold,
            "threshold.create",
            size,
            &[g.state.clone()],
            &GroupInstruction::Create {
                root: g.root,
                member_count: size as u32,
                threshold: 2,
                initial_value: 7,
            },
            &mut results,
        )?;
        g.apply_state(threshold.id(), &out, true);
        let out = run(
            &threshold,
            "threshold.propose",
            size,
            &[g.state.clone(), g.members[0].clone()],
            &GroupInstruction::Propose {
                witness: g.witnesses[0].clone(),
                next_value: 42,
            },
            &mut results,
        )?;
        g.apply_state(threshold.id(), &out, false);
        for member in 0..2 {
            let out = run(
                &threshold,
                if member == 0 {
                    "threshold.approve_first"
                } else {
                    "threshold.approve_second"
                },
                size,
                &[g.state.clone(), g.members[member].clone()],
                &GroupInstruction::Approve {
                    witness: g.witnesses[member].clone(),
                },
                &mut results,
            )?;
            g.apply_state(threshold.id(), &out, false);
        }
        g.state.is_authorized = false; // Permissionless execution after threshold.
        let out = run(
            &threshold,
            "threshold.execute",
            size,
            &[g.state.clone()],
            &GroupInstruction::Execute,
            &mut results,
        )?;
        g.apply_state(threshold.id(), &out, false);
        let state = Group::try_from_slice(&g.state.account.data)?;
        ensure!(
            state.value == 42 && state.proposal.context("proposal missing")?.executed,
            "threshold result incorrect"
        );
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"method":"actual RISC0 guest execution on synthetic benchmark inputs",
        "unit":"RISC0 user cycles, excluding continuation/po2 padding", "risc0_version":"3.0.5",
        "not_a_gas_price":true, "outer_privacy_circuit_excluded":true, "results":results})
        )?
    );
    Ok(())
}
