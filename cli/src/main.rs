//! Testnet-only JSON CLI. Private witness bytes are accepted only from protected
//! files inside the configured wallet. Nothing is sent to a public transaction
//! if the operation contains a member witness.
mod validation;
use anyhow::{Context, Result, ensure};
use borsh::BorshDeserialize;
use commons_logos_testnet_primitives::{
    Distribution, DistributionInstruction, Group, GroupInstruction, MemberWitness,
    execute_distribution, execute_group,
};
use lee::{privacy_preserving_transaction::circuit::ProgramWithDependencies, program::Program};
use lee_core::{
    account::{AccountId, AccountWithMetadata},
    program::ProgramId,
};
use sequencer_service_rpc::{RpcClient as _, SequencerClientBuilder};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    path::Path,
    time::Instant,
};
use validation::*;
use wallet::{AccountIdentity, WalletCore, config::WalletConfig};

/// A kernel-held lock is released on crash; no stale PID file needs deletion.
struct WalletLock(fs::File);
impl WalletLock {
    fn take(root: &Path) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join(".commons-cli.lock"))?;
        // SAFETY: a valid, held descriptor; no pointer or ownership transfer.
        ensure!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "WALLET_BUSY"
        );
        Ok(Self(file))
    }
}
impl Drop for WalletLock {
    fn drop(&mut self) {
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

fn decode_state(op: Op, bytes: &[u8]) -> Result<Value> {
    if op.family() == "allowlist" {
        let state = Distribution::try_from_slice(bytes)?;
        ensure!(state.magic == *b"COMNSD01", "wrong state type");
        Ok(
            json!({"root":hex::encode(state.root),"member_count":state.member_count,"claims_count":state.claims.len()}),
        )
    } else {
        let state = Group::try_from_slice(bytes)?;
        ensure!(state.magic == *b"COMNSM01", "wrong state type");
        Ok(
            json!({"root":hex::encode(state.root),"member_count":state.member_count,"threshold":state.threshold,"value":state.value.to_string(),"sequence":state.sequence,
   "proposal":state.proposal.map(|p|json!({"sequence":p.sequence,"next_value":p.next_value.to_string(),"approvals_count":p.approvals.len(),"executed":p.executed}))}),
        )
    }
}
async fn wallet(root: &Path) -> Result<WalletCore> {
    let metadata = fs::symlink_metadata(root.join("storage.json"))?;
    use std::os::unix::fs::PermissionsExt;
    ensure!(
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o077 == 0,
        "wallet storage must be owner-only regular file"
    );
    WalletCore::new_update_chain(
        root.join("config.json"),
        root.join("storage.json"),
        root.join("statistics.json"),
        None,
    )
    .await
}
fn pending(root: &Path, op: Op, state: AccountId, tx: &str) -> Result<()> {
    let path = root.join(".commons-pending.json");
    let tmp = root.join(".commons-pending.json.new");
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&tmp)?;
    file.write_all(&serde_json::to_vec(&json!({"operation":op.id(),"state_account":hex::encode(state.as_ref()),"tx_hash":tx,"status":"broadcast_unconfirmed"}))?)?;
    file.sync_all()?;
    fs::rename(tmp, path)?;
    Ok(())
}
async fn private<T: Serialize>(
    root: &Path,
    op: Op,
    w: &mut WalletCore,
    state: AccountId,
    witness: &MemberWitness,
    p: &ProgramWithDependencies,
    instruction: T,
) -> Result<(String, u64)> {
    let member = w
        .resolve_private_account(witness.leaf.account_id)
        .context("private member key unavailable")?;
    let (hash, _shared_keys) = w
        .send_privacy_preserving_tx(
            vec![AccountIdentity::PublicNoSign(state), member],
            Program::serialize_instruction(instruction)?,
            p,
        )
        .await?;
    pending(root, op, state, &hash.to_string())?;
    let (_, block) = w.poll_transaction(hash).await?;
    w.sync_to_latest_block().await?;
    w.store_persistent_data()?;
    fs::remove_file(root.join(".commons-pending.json"))?;
    Ok((hash.to_string(), block))
}
async fn public<T: Serialize>(
    root: &Path,
    op: Op,
    w: &mut WalletCore,
    state: AccountId,
    p: ProgramId,
    instruction: T,
) -> Result<(String, u64)> {
    // Execution is threshold-gated by program state, not by the creator's key.
    // v0.2.4 permits this public, signature-free relay once enough votes exist.
    let identity = if matches!(op, Op::Execute) {
        AccountIdentity::PublicNoSign(state)
    } else {
        AccountIdentity::Public(state)
    };
    let hash = w
        .send_pub_tx(
            vec![identity],
            Program::serialize_instruction(instruction)?,
            p,
        )
        .await?;
    pending(root, op, state, &hash.to_string())?;
    let (_, block) = w.poll_transaction(hash).await?;
    w.sync_to_latest_block().await?;
    w.store_persistent_data()?;
    fs::remove_file(root.join(".commons-pending.json"))?;
    Ok((hash.to_string(), block))
}
/// Resolve a previously broadcast transaction before allowing another write.
/// A network error or missing confirmation never deletes the checkpoint.
async fn reconcile_pending(root: &Path, w: &mut WalletCore) -> Result<()> {
    let path = root.join(".commons-pending.json");
    if !path.try_exists()? {
        return Ok(());
    }
    let saved: Value = serde_json::from_slice(&read_inside(root, &path, 8 * 1024, true)?)?;
    let tx = saved
        .get("tx_hash")
        .and_then(Value::as_str)
        .context("pending transaction identifier missing")?;
    validate_pending_tx_id(tx)?;
    let receipt = w.helm_owned().get_transaction(tx.parse()?).await?;
    ensure!(
        receipt.is_some(),
        "a previous transaction has not yet confirmed; refuse another write"
    );
    w.sync_to_latest_block().await?;
    w.store_persistent_data()?;
    fs::remove_file(path)?;
    Ok(())
}

async fn run(req: Request, op: Op) -> Result<Value> {
    validate(&req, op)?;
    let root = wallet_root(&req.wallet_dir)?;
    let config: WalletConfig = serde_json::from_slice(&read_inside(
        &root,
        &root.join("config.json"),
        64 * 1024,
        false,
    )?)?;
    let local = std::env::var("COMMONS_ALLOW_LOCAL_TESTNET").as_deref() == Ok("1");
    ensure!(
        config.sequencers.len() == 1,
        "exactly one testnet endpoint required"
    );
    let connection = &config.sequencers[0];
    let endpoint = connection.sequencer_addr.as_str();
    ensure!(
        connection.basic_auth.is_none() && endpoint_allowed(endpoint, local),
        "unexpected network or credential-bearing endpoint"
    );
    let client = SequencerClientBuilder::default()
        .request_timeout(std::time::Duration::from_secs(20))
        .build(endpoint)?;
    let ids = client.get_program_ids().await?;
    ensure!(
        ids.get("privacy_preserving_circuit") == Some(&PROTOCOL_ID),
        "testnet protocol fingerprint mismatch"
    );
    let programs: Value = serde_json::from_slice(&read_inside(
        &root,
        &root.join("programs.json"),
        16 * 1024,
        false,
    )?)?;
    let pid: ProgramId = serde_json::from_value(programs[op.family()].clone())?;
    let state_id = account(string(&req.arguments, "state_account")?)?;
    let chain_state = client.get_account(state_id).await?;
    if op.is_read() {
        ensure!(
            chain_state.program_owner == pid,
            "state owner does not match configured program"
        );
        let block = client.get_last_block_id().await?;
        return Ok(
            json!({"success":true,"operation":op.id(),"network":"testnet","endpoint":endpoint,"block_id":block,"program_id":pid,"state_account":hex::encode(state_id.as_ref()),"state":decode_state(op,&chain_state.data)?}),
        );
    }
    ensure!(
        std::env::var("RISC0_DEV_MODE").as_deref() == Ok("0"),
        "real proof mode required"
    );
    ensure!(
        std::env::var("RISC0_PROVER").as_deref() == Ok("ipc"),
        "explicit local IPC proving required; hosted proving is not supported"
    );
    let _lock = WalletLock::take(&root)?;
    let elf = read_inside(
        &root,
        &root
            .join("artifacts")
            .join(format!("commons_{}", op.family())),
        8 * 1024 * 1024,
        false,
    )?;
    let program = Program::new(elf.into())?;
    ensure!(
        program.id() == pid,
        "artifact image does not match deployment"
    );
    let p: ProgramWithDependencies = program.into();
    let mut w = wallet(&root).await?;
    reconcile_pending(&root, &mut w).await?;
    // Refresh keys/nonces only after taking the exclusive wallet lock.
    w.sync_to_latest_block().await?;
    let state = w.get_account_public(state_id).await?;
    let mut accounts = vec![AccountWithMetadata {
        account: state,
        is_authorized: w.get_account_public_signing_key(state_id).is_some(),
        account_id: state_id,
    }];
    let witness = if op.is_private() {
        let bytes = read_inside(
            &root,
            Path::new(string(&req.arguments, "witness_file")?),
            MAX_WITNESS,
            true,
        )?;
        let witness = MemberWitness::try_from_slice(&bytes)?;
        ensure!(
            w.resolve_private_account(witness.leaf.account_id).is_some(),
            "private key unavailable"
        );
        w.store_persistent_data()?;
        let member = if let Some(account) = w.get_account_private(witness.leaf.account_id) {
            account
        } else {
            // v0.2.4's convenience getter omits shared private accounts. Never turn an
            // existing shared account into a fake default state for preflight validation.
            let storage = wallet::storage::Storage::from_path(&root.join("storage.json"))?;
            storage
                .key_chain()
                .shared_private_account(witness.leaf.account_id)
                .map(|entry| entry.account.clone())
                .context("shared private member state unavailable")?
        };
        accounts.push(AccountWithMetadata {
            account: member,
            is_authorized: true,
            account_id: witness.leaf.account_id,
        });
        Some(witness)
    } else {
        None
    };
    let start = Instant::now();
    let (tx, block) = match op {
        Op::CreateDistribution => {
            let ix = DistributionInstruction::Create {
                root: hash(string(&req.arguments, "root")?)?,
                member_count: count(&req.arguments, "member_count")?,
            };
            execute_distribution(pid, &accounts, ix.clone())?;
            public(&root, op, &mut w, state_id, pid, ix).await?
        }
        Op::Claim => {
            let witness = witness.as_ref().unwrap();
            let ix = DistributionInstruction::Claim {
                witness: witness.clone(),
            };
            execute_distribution(pid, &accounts, ix.clone())?;
            private(&root, op, &mut w, state_id, witness, &p, ix).await?
        }
        Op::CreateGroup => {
            let ix = GroupInstruction::Create {
                root: hash(string(&req.arguments, "root")?)?,
                member_count: count(&req.arguments, "member_count")?,
                threshold: count(&req.arguments, "threshold")?,
                initial_value: integer(string(&req.arguments, "initial_value")?)?,
            };
            execute_group(pid, &accounts, ix.clone())?;
            public(&root, op, &mut w, state_id, pid, ix).await?
        }
        Op::Propose => {
            let witness = witness.as_ref().unwrap();
            let ix = GroupInstruction::Propose {
                witness: witness.clone(),
                next_value: integer(string(&req.arguments, "next_value")?)?,
            };
            execute_group(pid, &accounts, ix.clone())?;
            private(&root, op, &mut w, state_id, witness, &p, ix).await?
        }
        Op::Approve => {
            let witness = witness.as_ref().unwrap();
            let ix = GroupInstruction::Approve {
                witness: witness.clone(),
            };
            execute_group(pid, &accounts, ix.clone())?;
            private(&root, op, &mut w, state_id, witness, &p, ix).await?
        }
        Op::Execute => {
            let ix = GroupInstruction::Execute;
            execute_group(pid, &accounts, ix.clone())?;
            public(&root, op, &mut w, state_id, pid, ix).await?
        }
        _ => unreachable!("read-only modes returned before wallet mutation"),
    };
    let state = client.get_account(state_id).await?;
    Ok(
        json!({"success":true,"operation":op.id(),"network":"testnet","endpoint":endpoint,"program_id":pid,"tx_hash":tx,"block_id":block,"seconds":start.elapsed().as_secs_f64(),"private":op.is_private(),"risc0_dev_mode":"0","state_account":hex::encode(state_id.as_ref()),"state":decode_state(op,&state.data)?}),
    )
}
fn application_error_message(error: commons_logos_testnet_primitives::Error) -> &'static str {
    use commons_logos_testnet_primitives::Error::*;
    match error {
        AccountCount => "The operation received the wrong number of accounts.",
        Unauthorized => "This wallet cannot authorize that account.",
        AlreadyInitialized => "This account is already initialized. Choose an unused account.",
        WrongOwner => "The state account belongs to a different program.",
        InvalidState => {
            "The stored state could not be read. Check the account and program version."
        }
        InvalidSize => "The member count or membership proof is outside the supported range.",
        IdentityMismatch => "The credential does not match the member in this wallet.",
        InvalidMembership => {
            "This credential is not eligible for the selected group or distribution."
        }
        DuplicateClaim => "This member has already registered for this distribution.",
        CapacityReached => "All available registrations have been claimed.",
        InvalidThreshold => "Required approvals must be between one and the number of members.",
        PendingProposal => "Complete the current proposal before creating another.",
        NoProposal => "There is no proposal to approve or execute.",
        DuplicateApproval => "This member has already approved the current proposal.",
        ThresholdNotMet => "More member approvals are needed before this proposal can execute.",
        AlreadyExecuted => "This proposal has already executed.",
        ThresholdAlreadyMet => "The required approvals are present. The proposal can be executed.",
        SequenceOverflow => "The proposal counter is exhausted. Create a new group.",
        DataTooLarge => "The resulting state exceeds the account size limit.",
        InvalidViewingPublicKey => "The credential contains an invalid viewing key.",
        UninitializedMember => {
            "This member account cannot be reused with the selected program version."
        }
    }
}

fn safe_error(error: &anyhow::Error) -> Value {
    // Only static, enumerated messages cross the CLI boundary. Raw upstream
    // errors can contain witness fields, file paths or nested proof payloads.
    if let Some(e) = error.downcast_ref::<commons_logos_testnet_primitives::Error>() {
        return json!({"success":false,"error":{"code":*e as u32,"message":application_error_message(*e)}});
    }
    json!({"success":false,"error":{"code":"CLI_REQUEST_FAILED","message":"The client could not finish this operation. Check the connection and inspect the current testnet state before retrying."}})
}

#[cfg(test)]
mod public_error_tests {
    use super::*;
    use commons_logos_testnet_primitives::Error;

    #[test]
    fn maps_every_application_error_without_losing_its_code() {
        let cases = [
            Error::AccountCount,
            Error::Unauthorized,
            Error::AlreadyInitialized,
            Error::WrongOwner,
            Error::InvalidState,
            Error::InvalidSize,
            Error::IdentityMismatch,
            Error::InvalidMembership,
            Error::DuplicateClaim,
            Error::CapacityReached,
            Error::InvalidThreshold,
            Error::PendingProposal,
            Error::NoProposal,
            Error::DuplicateApproval,
            Error::ThresholdNotMet,
            Error::AlreadyExecuted,
            Error::ThresholdAlreadyMet,
            Error::SequenceOverflow,
            Error::DataTooLarge,
            Error::InvalidViewingPublicKey,
            Error::UninitializedMember,
        ];
        for e in cases {
            let output = safe_error(&anyhow::Error::new(e));
            assert_eq!(output["error"]["code"], e as u32);
            assert!(!output["error"]["message"].as_str().unwrap().is_empty());
            assert_eq!(output["success"], false);
        }
    }
    #[test]
    fn error_context_never_discloses_private_fields() {
        let secret = "SYNTHETIC_PRIVATE_CONTEXT_NOT_A_REAL_SECRET";
        let error = anyhow::Error::new(Error::DuplicateClaim).context(secret);
        let output = safe_error(&error).to_string();
        assert!(!output.contains(secret));
        assert!(output.contains("already registered"));
    }
    #[test]
    fn unknown_errors_do_not_echo_input_or_paths() {
        let error = anyhow::anyhow!("SYNTHETIC_CREDENTIAL /private/example/wallet");
        let output = safe_error(&error).to_string();
        assert!(!output.contains("SYNTHETIC_CREDENTIAL"));
        assert!(!output.contains("/private/example"));
        assert!(output.contains("CLI_REQUEST_FAILED"));
    }
}

fn main() {
    // Wallet dependencies print diagnostics to stdout. Redirect them BEFORE any
    // library call, preserving the original output descriptor only for our JSON.
    let out_fd = unsafe { libc::dup(libc::STDOUT_FILENO) };
    if out_fd < 0 {
        std::process::exit(2);
    }
    if unsafe { libc::dup2(libc::STDERR_FILENO, libc::STDOUT_FILENO) } < 0 {
        std::process::exit(2);
    }
    let mut output = unsafe { fs::File::from_raw_fd(out_fd) };
    let result = (|| -> Result<Value> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args == ["--help"] {
            return Ok(
                json!({"usage":"commons-logos-cli primitives <allowlist-create|allowlist-claim|allowlist-inspect|threshold-create|threshold-propose|threshold-approve|threshold-execute|threshold-inspect> --json-stdin","network":"testnet only","contract":"sdk/CLI-CONTRACT.md"}),
            );
        }
        ensure!(
            args.len() == 3 && args[0] == "primitives" && args[2] == "--json-stdin",
            "fixed command form required"
        );
        let op = Op::from_cli(&args[1])?;
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(MAX_REQUEST + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() as u64 <= MAX_REQUEST, "request exceeds bound");
        let request: Request = serde_json::from_slice(&bytes)?;
        validate(&request, op)?;
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()?;
        rt.block_on(run(request, op))
    })();
    let (status, value) = match result {
        Ok(value) => (0, value),
        Err(e) => (1, safe_error(&e)),
    };
    let _ = writeln!(output, "{}", value);
    let _ = output.flush();
    std::process::exit(status);
}
