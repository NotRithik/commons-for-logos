//! Testnet-only JSON CLI. Private witness bytes are accepted only from protected
//! files inside the configured wallet. Nothing is sent to a public transaction
//! if the operation contains a member witness.
mod execution_intent;
mod governance;
mod validation;
use anyhow::{Context, Result, ensure};
use borsh::BorshDeserialize;
use commons_logos_testnet_primitives::{
    Distribution, DistributionInstruction, Group, GroupInstruction, MemberWitness,
    execute_distribution, execute_group,
};
use execution_intent::ExecutionIntent;
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

/// Only these stable categories cross the UI boundary. Upstream errors may
/// contain local paths or private witness data and are never displayed verbatim.
#[derive(Debug, Clone, Copy)]
enum ClientIssue {
    NetworkUnavailable,
    ProtocolChanged,
    DeploymentMissing,
    DifferentProgram,
    WalletHistoryAhead,
    ProfileUnreadable,
    ReadOnlyWorkspace,
    ReviewChanged,
    ExecutionUnresolved,
    LocalProverUnavailable,
    CredentialUnreadable,
    WalletBusy,
}
impl std::fmt::Display for ClientIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for ClientIssue {}
impl ClientIssue {
    fn public(self) -> (&'static str, &'static str) {
        match self {
            Self::LocalProverUnavailable => (
                "LOCAL_PROVER_UNAVAILABLE",
                "The configured local proof engine is missing or not executable. No transaction was submitted by this attempt. Restore the matching proof engine, then refresh and review the action again.",
            ),
            Self::CredentialUnreadable => (
                "MEMBERSHIP_CREDENTIAL_UNREADABLE",
                "This membership credential cannot be read. Import your own original invitation again or select the correct protected credential file, then refresh before retrying.",
            ),
            Self::WalletBusy => (
                "WALLET_BUSY",
                "This member wallet is already in use by another operation. Wait for its result before trying again. Do not delete wallet lock or pending files.",
            ),
            Self::ReviewChanged => (
                "REVIEWED_STATE_CHANGED",
                "The policy changed after you reviewed it. No new transaction was sent. Refresh the decision and review the current proposal again.",
            ),
            Self::ExecutionUnresolved => (
                "EXECUTION_RECONCILIATION_REQUIRED",
                "The earlier execution receipt exists, but its exact result could not be verified from the current state. No transaction was repeated. Preserve the pending record for reconciliation.",
            ),
            Self::NetworkUnavailable => (
                "NETWORK_UNAVAILABLE",
                "The testnet is not responding. Your wallet has not been changed. Check your connection and try Refresh.",
            ),
            Self::ProtocolChanged => (
                "TESTNET_PROTOCOL_CHANGED",
                "The testnet uses a different proof format. Update this client before sending an action; your existing wallet is preserved.",
            ),
            Self::DeploymentMissing => (
                "DEPLOYMENT_NOT_FOUND",
                "This workspace is not present on the current testnet. It may not have been created yet, or the testnet may have restarted. Choose a current workspace; old receipts are historical only.",
            ),
            Self::DifferentProgram => (
                "ACCOUNT_PROGRAM_MISMATCH",
                "This account belongs to a different application. Choose the correct group or check the account address.",
            ),
            Self::WalletHistoryAhead => (
                "TESTNET_HISTORY_CHANGED",
                "The testnet restarted after this wallet was used. Keep this wallet as an archive and use a fresh testnet workspace. No old transaction was replayed.",
            ),
            Self::ProfileUnreadable => (
                "PROFILE_UNREADABLE",
                "The selected workspace is incomplete or cannot be read. Choose another saved workspace or check its setup in Connection settings.",
            ),
            Self::ReadOnlyWorkspace => (
                "READ_ONLY_WORKSPACE",
                "This is a viewing-only workspace. Connect your own member wallet to submit an action.",
            ),
        }
    }
}

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
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(ClientIssue::WalletBusy.into());
        }
        Ok(Self(file))
    }
}
impl Drop for WalletLock {
    fn drop(&mut self) {
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

// Constant stage identifiers only; never include witnesses, member IDs or paths.
fn progress(stage: &str) {
    eprintln!("\nCOMMONS_PROGRESS_V1 {stage}");
}

fn check_configured_prover() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    // An explicitly configured absolute engine path can be checked without
    // spawning it or opening a member wallet. Leave upstream PATH discovery
    // intact when no absolute path has been configured.
    if let Some(raw) = std::env::var_os("RISC0_SERVER_PATH") {
        let path = std::path::PathBuf::from(raw);
        if path.as_os_str().is_empty() {
            return Err(ClientIssue::LocalProverUnavailable.into());
        }
        if path.is_absolute() {
            let metadata = fs::metadata(&path).context(ClientIssue::LocalProverUnavailable)?;
            if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
                return Err(ClientIssue::LocalProverUnavailable.into());
            }
        }
    }
    Ok(())
}

fn state_fingerprint(bytes: &[u8]) -> String {
    hex::encode(commons_logos_testnet_primitives::hash_parts(
        b"commons/reviewed-state/v1",
        &[bytes],
    ))
}
fn check_review(req: &Request, bytes: &[u8]) -> Result<()> {
    if let Some(expected) = req.arguments.get("expected_state_fingerprint") {
        if hash(
            expected
                .as_str()
                .context("review fingerprint must be a string")?,
        )? != hash(&state_fingerprint(bytes))?
        {
            return Err(ClientIssue::ReviewChanged.into());
        }
    }
    Ok(())
}

fn decode_state(op: Op, bytes: &[u8]) -> Result<Value> {
    if op.family() == "allowlist" {
        let state = Distribution::try_from_slice(bytes)?;
        ensure!(state.magic == *b"COMNSD01", "wrong state type");
        Ok(
            json!({"fingerprint":state_fingerprint(bytes),"root":hex::encode(state.root),"member_count":state.member_count,"claims_count":state.claims.len()}),
        )
    } else {
        let state = Group::try_from_slice(bytes)?;
        ensure!(state.magic == *b"COMNSM01", "wrong state type");
        Ok(
            json!({"fingerprint":state_fingerprint(bytes),"root":hex::encode(state.root),"member_count":state.member_count,"threshold":state.threshold,"value":state.value.to_string(),"sequence":state.sequence,
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
fn pending(
    root: &Path,
    op: Op,
    state: AccountId,
    tx: &str,
    intent: Option<ExecutionIntent>,
) -> Result<()> {
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
    file.write_all(&serde_json::to_vec(&json!({"operation":op.id(),"state_account":hex::encode(state.as_ref()),"tx_hash":tx,"status":"broadcast_unconfirmed","execution_intent":intent.map(|i|json!({"sequence":i.sequence,"next_value":i.next_value}))}))?)?;
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
    expected_state: &lee_core::account::Account,
) -> Result<(String, u64)> {
    let member = w
        .resolve_private_account(witness.leaf.account_id)
        .context("private member key unavailable")?;
    progress("proving");
    let changed = std::cell::Cell::new(false);
    let sent = w
        .send_privacy_preserving_tx_with_pre_check(
            vec![AccountIdentity::PublicNoSign(state), member],
            Program::serialize_instruction(instruction)?,
            p,
            |states| {
                if states.first().copied() != Some(expected_state) {
                    changed.set(true);
                    return Err(wallet::ExecutionFailureKind::TransactionBuildError(
                        lee::error::LeeError::InvalidInput("reviewed public state changed".into()),
                    ));
                }
                Ok(())
            },
        )
        .await;
    if changed.get() {
        return Err(ClientIssue::ReviewChanged.into());
    }
    let (hash, _shared_keys) = sent?;
    pending(root, op, state, &hash.to_string(), None)?;
    progress("confirming");
    let (_, block) = w.poll_transaction(hash).await?;
    progress("syncing-result");
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
    progress("submitting");
    let hash = w
        .send_pub_tx(
            vec![identity],
            Program::serialize_instruction(instruction)?,
            p,
        )
        .await?;
    pending(root, op, state, &hash.to_string(), None)?;
    progress("confirming");
    let (_, block) = w.poll_transaction(hash).await?;
    w.sync_to_latest_block().await?;
    w.store_persistent_data()?;
    fs::remove_file(root.join(".commons-pending.json"))?;
    Ok((hash.to_string(), block))
}
/// A public, signature-free relay with deterministic per-proposal transport
/// identity. Unlike the old unit Execute transaction, later proposals cannot
/// resolve to a previously accepted transaction hash.
async fn execute_public(
    root: &Path,
    w: &mut WalletCore,
    state: AccountId,
    program: ProgramId,
    intent: ExecutionIntent,
) -> Result<(String, u64)> {
    use common::transaction::LeeTransaction;
    use lee::public_transaction::{Message, PublicTransaction, WitnessSet};
    let message =
        Message::new_preserialized(program, vec![state], vec![], intent.instruction_words()?);
    let transaction = PublicTransaction::new(message, WitnessSet::from_raw_parts(vec![]));
    let expected: common::HashType = transaction.hash().into();
    // Record exact intent BEFORE network submission. An ambiguous response keeps
    // this checkpoint and must be reconciled rather than generating a new tag.
    pending(
        root,
        Op::Execute,
        state,
        &expected.to_string(),
        Some(intent),
    )?;
    progress("submitting");
    let returned = w
        .helm_owned()
        .send_transaction(LeeTransaction::Public(transaction.clone()))
        .await?;
    ensure!(
        returned == expected,
        "sequencer returned a different transaction hash"
    );
    progress("confirming");
    let (observed, block) = w.poll_transaction(expected).await?;
    ensure!(
        observed == LeeTransaction::Public(transaction),
        "confirmed transaction differs from prepared execution"
    );
    w.sync_to_latest_block().await?;
    w.store_persistent_data()?;
    // Keep checkpoint until the exact postcondition has been independently read.
    Ok((expected.to_string(), block))
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
    if saved.get("operation").and_then(Value::as_str) == Some("threshold.execute") {
        let target = account(
            saved["state_account"]
                .as_str()
                .context(ClientIssue::ExecutionUnresolved)?,
        )?;
        let intent = ExecutionIntent {
            sequence: saved["execution_intent"]["sequence"]
                .as_u64()
                .context(ClientIssue::ExecutionUnresolved)?,
            next_value: saved["execution_intent"]["next_value"]
                .as_i64()
                .context(ClientIssue::ExecutionUnresolved)?,
        };
        let programs: Value = serde_json::from_slice(&read_inside(
            root,
            &root.join("programs.json"),
            16 * 1024,
            false,
        )?)?;
        let expected_program: ProgramId = serde_json::from_value(programs["threshold"].clone())?;
        let current = w.get_account_public(target).await?;
        if current.program_owner != expected_program {
            return Err(ClientIssue::ExecutionUnresolved.into());
        }
        intent
            .verify_postcondition(&Group::try_from_slice(&current.data)?)
            .context(ClientIssue::ExecutionUnresolved)?;
    }
    fs::remove_file(path)?;
    Ok(())
}

fn check_workspace_access(root: &Path, op: Op) -> Result<()> {
    if op.is_read() {
        return Ok(());
    }
    match fs::symlink_metadata(root.join(".commons-readonly")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(ClientIssue::ReadOnlyWorkspace.into()),
    }
}

async fn run(req: Request, op: Op) -> Result<Value> {
    validate(&req, op)?;
    let root = wallet_root(&req.wallet_dir).context(ClientIssue::ProfileUnreadable)?;
    // Enforce read-only mode at the CLI boundary, not merely in the GUI.
    check_workspace_access(&root, op)?;
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
    let ids = client
        .get_program_ids()
        .await
        .context(ClientIssue::NetworkUnavailable)?;
    if ids.get("privacy_preserving_circuit") != Some(&PROTOCOL_ID) {
        return Err(ClientIssue::ProtocolChanged.into());
    }
    let programs: Value = serde_json::from_slice(&read_inside(
        &root,
        &root.join("programs.json"),
        16 * 1024,
        false,
    )?)?;
    let pid: ProgramId = serde_json::from_value(programs[op.family()].clone())?;
    let state_id = account(string(&req.arguments, "state_account")?)?;
    let chain_state = client
        .get_account(state_id)
        .await
        .context(ClientIssue::NetworkUnavailable)?;
    if op.is_read() {
        if chain_state.program_owner == [0; 8] && chain_state.data.is_empty() {
            return Err(ClientIssue::DeploymentMissing.into());
        }
        if chain_state.program_owner != pid {
            return Err(ClientIssue::DifferentProgram.into());
        }
        let block = client
            .get_last_block_id()
            .await
            .context(ClientIssue::NetworkUnavailable)?;
        return Ok(
            json!({"success":true,"operation":op.id(),"network":"testnet","endpoint":endpoint,"block_id":block,"program_id":pid,"state_account":hex::encode(state_id.as_ref()),"state":decode_state(op,&chain_state.data)?}),
        );
    }
    check_review(&req, &chain_state.data)?;
    ensure!(
        std::env::var("RISC0_DEV_MODE").as_deref() == Ok("0"),
        "real proof mode required"
    );
    ensure!(
        std::env::var("RISC0_PROVER").as_deref() == Ok("ipc"),
        "explicit local IPC proving required; hosted proving is not supported"
    );
    if op.is_private() {
        check_configured_prover()?;
    }
    // A reset can retain the same proof ABI while erasing account history.
    // Never silently rewind a wallet or reuse its previous private witnesses.
    let latest = client
        .get_last_block_id()
        .await
        .context(ClientIssue::NetworkUnavailable)?;
    let stored = wallet::storage::Storage::from_path(&root.join("storage.json"))
        .context(ClientIssue::ProfileUnreadable)?;
    if stored.last_synced_block() > latest {
        return Err(ClientIssue::WalletHistoryAhead.into());
    }
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
    progress("syncing");
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
        )
        .context(ClientIssue::CredentialUnreadable)?;
        let witness =
            MemberWitness::try_from_slice(&bytes).context(ClientIssue::CredentialUnreadable)?;
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
    check_review(&req, &accounts[0].account.data)?;
    let execution_intent = if matches!(op, Op::Execute) {
        Some(ExecutionIntent::from_group(&Group::try_from_slice(
            &accounts[0].account.data,
        )?)?)
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
            private(
                &root,
                op,
                &mut w,
                state_id,
                witness,
                &p,
                ix,
                &accounts[0].account,
            )
            .await?
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
            private(
                &root,
                op,
                &mut w,
                state_id,
                witness,
                &p,
                ix,
                &accounts[0].account,
            )
            .await?
        }
        Op::Approve => {
            let witness = witness.as_ref().unwrap();
            let ix = GroupInstruction::Approve {
                witness: witness.clone(),
            };
            execute_group(pid, &accounts, ix.clone())?;
            private(
                &root,
                op,
                &mut w,
                state_id,
                witness,
                &p,
                ix,
                &accounts[0].account,
            )
            .await?
        }
        Op::Execute => {
            let ix = GroupInstruction::Execute;
            execute_group(pid, &accounts, ix)?;
            execute_public(
                &root,
                &mut w,
                state_id,
                pid,
                execution_intent.context("execution intent missing")?,
            )
            .await?
        }
        _ => unreachable!("read-only modes returned before wallet mutation"),
    };
    let state = client.get_account(state_id).await?;
    if let Some(intent) = execution_intent {
        ensure!(
            state.program_owner == pid,
            "execution result program owner changed"
        );
        intent.verify_postcondition(&Group::try_from_slice(&state.data)?)?;
        fs::remove_file(root.join(".commons-pending.json"))?;
    }
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
    if let Some(issue) = error.downcast_ref::<governance::PublicIssue>() {
        return json!({"success":false,"error":{"code":"GOVERNANCE_SETUP_FAILED","message":issue.0}});
    }

    // Only static, enumerated messages cross the CLI boundary. Raw upstream
    // errors can contain witness fields, file paths or nested proof payloads.
    if let Some(e) = error.downcast_ref::<commons_logos_testnet_primitives::Error>() {
        return json!({"success":false,"error":{"code":*e as u32,"message":application_error_message(*e)}});
    }
    if matches!(
        error.downcast_ref::<wallet::ExecutionFailureKind>(),
        Some(wallet::ExecutionFailureKind::TransactionBuildError(
            lee::error::LeeError::CircuitProvingError(_)
        ))
    ) {
        // In the pinned wallet this typed failure precedes transaction send.
        // Other errors, especially send/timeout errors, may be ambiguous.
        return json!({"success":false,"error":{"code":"LOCAL_PROOF_FAILED","message":"The private proof could not be generated locally. No transaction was submitted by this attempt. Check the local proof dependencies, then refresh and review the action again."}});
    }
    if let Some(issue) = error.downcast_ref::<ClientIssue>() {
        let (code, message) = issue.public();
        return json!({"success":false,"error":{"code":code,"message":message}});
    }
    json!({"success":false,"error":{"code":"CLI_REQUEST_FAILED","message":"The client could not finish this operation. Check the connection and inspect the current testnet state before retrying."}})
}

#[cfg(test)]
mod public_error_tests {
    use super::*;
    use commons_logos_testnet_primitives::Error;

    #[test]
    fn read_only_workspace_allows_reads_and_rejects_writes_before_network() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("commons-readonly-{}-{stamp}", std::process::id()));
        fs::create_dir(&root).unwrap();
        assert!(check_workspace_access(&root, Op::CreateDistribution).is_ok());
        fs::write(root.join(".commons-readonly"), b"viewer").unwrap();
        assert!(check_workspace_access(&root, Op::InspectDistribution).is_ok());
        let error = check_workspace_access(&root, Op::CreateDistribution).unwrap_err();
        assert_eq!(safe_error(&error)["error"]["code"], "READ_ONLY_WORKSPACE");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reset_and_network_errors_are_specific_without_upstream_secrets() {
        for (issue, expected) in [
            (ClientIssue::NetworkUnavailable, "NETWORK_UNAVAILABLE"),
            (ClientIssue::ProtocolChanged, "TESTNET_PROTOCOL_CHANGED"),
            (ClientIssue::DeploymentMissing, "DEPLOYMENT_NOT_FOUND"),
            (ClientIssue::DifferentProgram, "ACCOUNT_PROGRAM_MISMATCH"),
            (ClientIssue::WalletHistoryAhead, "TESTNET_HISTORY_CHANGED"),
            (ClientIssue::ProfileUnreadable, "PROFILE_UNREADABLE"),
        ] {
            let error = anyhow::anyhow!("private witness and /private/wallet should not appear")
                .context(issue);
            let value = safe_error(&error);
            assert_eq!(value["error"]["code"], expected);
            assert!(!value.to_string().contains("/private/wallet"));
            assert!(!value.to_string().contains("should not appear"));
        }
    }

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
        if args.len() == 3 && args[0] == "governance" && args[2] == "--json-stdin" {
            let mut bytes = Vec::new();
            std::io::stdin()
                .take(256 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= 256 * 1024,
                "governance request exceeds bound"
            );
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .build()?;
            return rt
                .block_on(governance::dispatch(&args[1], &bytes))
                .map_err(|error| anyhow::anyhow!(governance::public_issue(&error)));
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
