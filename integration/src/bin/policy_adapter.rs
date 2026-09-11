//! Public-only, resumable real-network consumer demonstration.
//! No member wallet is opened and no private proposal, approval, funding or claim is sent.
use anyhow::{Context, Result, ensure};
use borsh::BorshDeserialize;
use commons_logos_policy_adapter::{
    AdapterError, GovernedSetting, GovernedSettingInstruction as Instruction, PolicyBinding,
    THRESHOLD_PROGRAM_ID, execute_governed_setting, read_executed_decision,
};
use lee::program::Program;
use lee_core::{
    account::{AccountId, AccountWithMetadata},
    program::{ProgramId, ProgramOutput},
};
use sequencer_service_rpc::{RpcClient as _, SequencerClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::Instant,
};
use wallet::{
    AccountIdentity, WalletCore,
    config::{SequencerConnectionData, WalletConfig},
    storage::Storage,
};

const ENDPOINT: &str = "https://testnet.lez.logos.co";
const PROTOCOL: ProgramId = [
    1334328888, 3910590567, 1244219104, 3671232111, 3138827701, 405554639, 4064616947, 1864368340,
];

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct Plan {
    schema_version: u32,
    program_id: ProgramId,
    policy: AccountId,
    consumer: AccountId,
    expected_sequence: u64,
    expected_value: i64,
}
struct Lock(fs::File);
impl Lock {
    fn take(root: &Path) -> Result<Self> {
        let f = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join(".adapter.lock"))?;
        ensure!(
            unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "Adapter demonstration already running"
        );
        Ok(Self(f))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}
fn write_new(path: &Path, value: &Value) -> Result<()> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    f.write_all(&serde_json::to_vec_pretty(value)?)?;
    f.sync_all()?;
    Ok(())
}
fn replace(path: &Path, value: &Value) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    write_new(&temp, value)?;
    fs::rename(&temp, path)?;
    fs::File::open(path.parent().context("Missing parent")?)?.sync_all()?;
    Ok(())
}
fn load(path: &Path) -> Result<Value> {
    let m = fs::symlink_metadata(path)?;
    ensure!(
        m.is_file() && !m.file_type().is_symlink() && m.len() < 1024 * 1024,
        "Invalid adapter record"
    );
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn event(root: &Path, out: &mut fs::File, value: Value) -> Result<()> {
    writeln!(out, "{value}")?;
    out.flush()?;
    let mut f = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join("public-events.jsonl"))?;
    writeln!(f, "{value}")?;
    f.sync_data()?;
    Ok(())
}
fn guest(
    program: &Program,
    accounts: &[AccountWithMetadata],
    instruction: &Instruction,
) -> Result<(Vec<lee_core::program::AccountPostState>, u64)> {
    let words = Program::serialize_instruction(instruction)?;
    let mut b = risc0_zkvm::ExecutorEnv::builder();
    b.session_limit(Some(32 * 1024 * 1024));
    b.write(&program.id())?
        .write(&Option::<ProgramId>::None)?
        .write(&accounts)?
        .write(&words)?;
    let session = risc0_zkvm::default_executor().execute(b.build()?, program.elf())?;
    let output: ProgramOutput = session.journal.decode()?;
    ensure!(
        output.pre_states == accounts,
        "Guest pre-state binding mismatch"
    );
    lee_core::program::validate_execution(accounts, &output.post_states, program.id())?;
    ensure!(
        output.post_states.len() == 2 && output.post_states[1].account() == &accounts[1].account,
        "Consumer changed its foreign policy input"
    );
    let cycles = session.segments.iter().map(|s| u64::from(s.cycles)).sum();
    Ok((output.post_states, cycles))
}
async fn prepare(
    root: &Path,
    program: ProgramId,
    policy: AccountId,
    seq: u64,
    value: i64,
) -> Result<Plan> {
    let file = root.join("plan.json");
    if file.exists() {
        let p: Plan = serde_json::from_value(load(&file)?)?;
        ensure!(
            p.schema_version == 1
                && p.program_id == program
                && p.policy == policy
                && p.expected_sequence == seq
                && p.expected_value == value,
            "Existing adapter intent differs; no file or transaction was replaced"
        );
        return Ok(p);
    }
    ensure!(
        !root.join("storage.json").exists(),
        "Incomplete existing wallet; inspect it instead of replacing its keys"
    );
    let rpc = SequencerClientBuilder::default()
        .request_timeout(std::time::Duration::from_secs(25))
        .build(ENDPOINT)?;
    ensure!(
        rpc.get_program_ids()
            .await?
            .get("privacy_preserving_circuit")
            == Some(&PROTOCOL),
        "Incompatible testnet protocol"
    );
    let birth = rpc.get_last_block_id().await?;
    let (mut storage, _mnemonic) = Storage::new("public-only-adapter-fixture-not-encrypted")?;
    let consumer = storage
        .key_chain_mut()
        .generate_new_public_transaction_private_key(None)
        .0;
    storage.set_last_synced_block(birth); // Only these newly generated keys use this checkpoint.
    write_new(&root.join("storage.json"), &json!({}))?;
    storage.save_to_path(&root.join("storage.json"))?;
    fs::set_permissions(root.join("storage.json"), fs::Permissions::from_mode(0o600))?;
    let mut config = WalletConfig::default();
    config.sequencers = vec![SequencerConnectionData {
        sequencer_addr: ENDPOINT.parse()?,
        basic_auth: None,
    }];
    config.seq_tx_poll_max_blocks = 180;
    config.seq_poll_timeout = std::time::Duration::from_millis(500);
    config.multi_sequencer_client_config.calibration_limit = 2;
    write_new(&root.join("config.json"), &serde_json::to_value(config)?)?;
    write_new(&root.join("statistics.json"), &json!({}))?;
    let p = Plan {
        schema_version: 1,
        program_id: program,
        policy,
        consumer,
        expected_sequence: seq,
        expected_value: value,
    };
    write_new(&file, &serde_json::to_value(&p)?)?;
    Ok(p)
}
async fn confirm(w: &WalletCore, root: &Path, stage: &str, out: &mut fs::File) -> Result<()> {
    let file = root.join(format!("{stage}.json"));
    let mut record = load(&file)?;
    if record["status"] == "confirmed" {
        return Ok(());
    }
    let hash=record["tx_hash"].as_str().context("Broadcast outcome unknown. Inspect this saved intent and the chain; it must not be automatically replayed")?.parse()?;
    let (_, block) = w.poll_transaction(hash).await?;
    record["status"] = json!("confirmed");
    record["block_id"] = json!(block);
    replace(&file, &record)?;
    event(root, out, record)
}
async fn deploy(
    w: &WalletCore,
    root: &Path,
    bytes: Vec<u8>,
    program: ProgramId,
    out: &mut fs::File,
) -> Result<()> {
    let file = root.join("deploy.json");
    if !file.exists() {
        write_new(
            &file,
            &json!({"stage":"adapter.deploy","status":"broadcast-intent","program_id":program}),
        )?;
        let hash = w.send_program_deployment_transaction(bytes).await?;
        replace(
            &file,
            &json!({"stage":"adapter.deploy","status":"pending","program_id":program,"tx_hash":hash.to_string()}),
        )?;
    }
    confirm(w, root, "deploy", out).await
}
async fn call(
    w: &WalletCore,
    root: &Path,
    stage: &str,
    plan: &Plan,
    program: &Program,
    instruction: Instruction,
    initialize: bool,
    out: &mut fs::File,
) -> Result<()> {
    let file = root.join(format!("{stage}.json"));
    if file.exists() {
        return confirm(w, root, stage, out).await;
    }
    let accounts = vec![
        AccountWithMetadata::new(
            w.get_account_public(plan.consumer).await?,
            initialize,
            plan.consumer,
        ),
        AccountWithMetadata::new(w.get_account_public(plan.policy).await?, false, plan.policy),
    ];
    let (_, cycles) = guest(program, &accounts, &instruction)?;
    event(
        root,
        out,
        json!({"stage":format!("adapter.{stage}.guest"),"status":"executed-real-guest","guest_user_cycles":cycles,"proof":false,"foreign_policy_unchanged":true}),
    )?;
    let words = Program::serialize_instruction(&instruction)?;
    write_new(
        &file,
        &json!({"stage":format!("adapter.{stage}"),"status":"broadcast-intent","program_id":program.id(),"consumer":plan.consumer,"policy":plan.policy,"instruction":instruction}),
    )?;
    let started = Instant::now();
    let identity = if initialize {
        AccountIdentity::Public(plan.consumer)
    } else {
        AccountIdentity::PublicNoSign(plan.consumer)
    };
    let hash = w
        .send_pub_tx(
            vec![identity, AccountIdentity::PublicNoSign(plan.policy)],
            words,
            program.id(),
        )
        .await?;
    replace(
        &file,
        &json!({"stage":format!("adapter.{stage}"),"status":"pending","tx_hash":hash.to_string(),"private":false,"program_id":program.id(),"consumer":plan.consumer,"policy":plan.policy,"submission_seconds":started.elapsed().as_secs_f64()}),
    )?;
    confirm(w, root, stage, out).await
}
async fn run(args: &[String], out: &mut fs::File) -> Result<()> {
    ensure!(
        args.len() == 6 && args[0] == "run",
        "Usage: policy_adapter run ABSOLUTE_NEW_DIRECTORY PACKED_GUEST POLICY_BASE58 SEQUENCE VALUE"
    );
    ensure!(
        std::env::var("COMMONS_ALLOW_PUBLIC_TESTNET").as_deref() == Ok("1"),
        "Set COMMONS_ALLOW_PUBLIC_TESTNET=1 to authorize public testnet deployment and consumer transactions"
    );
    ensure!(
        std::env::var("RISC0_DEV_MODE").as_deref() == Ok("0"),
        "RISC0_DEV_MODE=0 required"
    );
    ensure!(
        std::env::var("RISC0_EXECUTOR").as_deref() == Ok("ipc"),
        "Local IPC executor required"
    );
    let root = PathBuf::from(&args[1]);
    ensure!(
        root.is_absolute(),
        "An absolute isolated directory is required"
    );
    if !root.exists() {
        fs::create_dir(&root)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?
    }
    let metadata = fs::symlink_metadata(&root)?;
    ensure!(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o077 == 0,
        "Use a private regular directory (mode 700)"
    );
    let _lock = Lock::take(&root)?;
    let bytes = fs::read(&args[2])?;
    ensure!(bytes.len() < 8 * 1024 * 1024, "Oversized guest artifact");
    let program = Program::new(bytes.clone().into())?;
    let policy: AccountId = args[3].parse()?;
    let seq: u64 = args[4].parse()?;
    let value: i64 = args[5].parse()?;
    ensure!(seq > 0, "Expected an executed proposal sequence");
    let plan = prepare(&root, program.id(), policy, seq, value).await?;
    let config: WalletConfig = serde_json::from_value(load(&root.join("config.json"))?)?;
    ensure!(
        config.sequencers.len() == 1
            && config.sequencers[0].sequencer_addr.as_str() == "https://testnet.lez.logos.co/"
            && config.sequencers[0].basic_auth.is_none(),
        "Only the official testnet is allowed"
    );
    let rpc = SequencerClientBuilder::default()
        .request_timeout(std::time::Duration::from_secs(25))
        .build(ENDPOINT)?;
    ensure!(
        rpc.get_program_ids()
            .await?
            .get("privacy_preserving_circuit")
            == Some(&PROTOCOL),
        "Incompatible current testnet protocol"
    );
    let w = WalletCore::new_update_chain(
        root.join("config.json"),
        root.join("storage.json"),
        root.join("statistics.json"),
        None,
    )
    .await?;
    let policy_before = w.get_account_public(policy).await?;
    let binding = PolicyBinding {
        program_id: THRESHOLD_PROGRAM_ID,
        state_account: policy,
    };
    let decision = read_executed_decision(
        &AccountWithMetadata::new(policy_before.clone(), false, policy),
        &binding,
    )?;
    ensure!(
        decision.sequence == seq && decision.value == value,
        "Live executed policy differs from the reviewed request"
    );
    event(
        &root,
        out,
        json!({"stage":"adapter.review","network":ENDPOINT,"RISC0_DEV_MODE":"0","program_id":program.id(),"policy":policy,"consumer":plan.consumer,"sequence":seq,"value":value,"member_wallets_loaded":false,"token_transfers":0}),
    )?;
    deploy(&w, &root, bytes, program.id(), out).await?;
    call(
        &w,
        &root,
        "initialize",
        &plan,
        &program,
        Instruction::Initialize {
            policy: binding.clone(),
            minimum: i64::MIN,
            maximum: i64::MAX,
            initial_value: 0,
        },
        true,
        out,
    )
    .await?;
    call(
        &w,
        &root,
        "apply",
        &plan,
        &program,
        Instruction::Apply {
            expected_sequence: seq,
            expected_value: value,
        },
        false,
        out,
    )
    .await?;
    let own = w.get_account_public(plan.consumer).await?;
    let policy_after = w.get_account_public(policy).await?;
    ensure!(own.program_owner == program.id(), "Consumer owner mismatch");
    let state = GovernedSetting::try_from_slice(&own.data)?;
    ensure!(
        state.policy == binding && state.last_consumed_sequence == seq && state.value == value,
        "Exact consumer postcondition not met"
    );
    ensure!(
        policy_after.data == policy_before.data
            && policy_after.program_owner == policy_before.program_owner
            && policy_after.balance == policy_before.balance,
        "Foreign policy changed during demonstration; review concurrent actions"
    );
    let pre = vec![
        AccountWithMetadata::new(own, false, plan.consumer),
        AccountWithMetadata::new(policy_after, false, policy),
    ];
    let replay = Instruction::Apply {
        expected_sequence: seq,
        expected_value: value,
    };
    ensure!(
        execute_governed_setting(program.id(), &pre, replay.clone())
            == Err(AdapterError::DecisionAlreadyConsumed),
        "Host decoder failed to reject replay"
    );
    let replay_error = guest(&program, &pre, &replay)
        .err()
        .context("Actual guest did not reject replay")?;
    ensure!(
        format!("{replay_error:#}").contains("COMMONS_ADAPTER_2006_DecisionAlreadyConsumed"),
        "Guest failed for a different reason; replay protection was not verified"
    );
    let report = json!({"stage":"adapter.complete","success":true,"network":ENDPOINT,"block_id":w.get_last_block_id().await?,"program_id":program.id(),"policy":policy,"consumer":plan.consumer,"value":value,"last_consumed_sequence":seq,"foreign_policy_application_state_unchanged":true,"replay_rejected_by_real_guest":true,"replay_transaction_sent":false,"private_proofs_repeated":0,"token_transfers":0});
    replace(&root.join("public-report.json"), &report)?;
    event(&root, out, report)?;
    Ok(())
}
fn main() {
    let fd = unsafe { libc::dup(libc::STDOUT_FILENO) };
    if fd < 0 {
        std::process::exit(2)
    }
    // Dependency diagnostics are not public evidence. Discard stdout before opening a wallet.
    let sink = fs::OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .expect("null device");
    if unsafe { libc::dup2(sink.as_raw_fd(), libc::STDOUT_FILENO) } < 0 {
        std::process::exit(2)
    }
    let mut out = unsafe { fs::File::from_raw_fd(fd) };
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        let _ = writeln!(
            out,
            "policy_adapter run ABSOLUTE_NEW_DIRECTORY PACKED_GUEST POLICY_BASE58 SEQUENCE VALUE\nPublic testnet only; no member wallet or private approval is used. Requires COMMONS_ALLOW_PUBLIC_TESTNET=1 and RISC0_DEV_MODE=0. Reuse the exact directory/intent to reconcile recorded transaction hashes, never to replace a wallet."
        );
        return;
    }
    let result = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .and_then(|rt| Ok(rt.block_on(run(&args, &mut out))));
    match result {
        Ok(Ok(())) => (),
        failure => {
            if let Some(root) = args.get(1).map(PathBuf::from) {
                let filename = format!(
                    "private-failure-{}.txt",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                );
                if let Ok(mut file) = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(root.join(filename))
                {
                    match failure {
                        Ok(Err(error)) => {
                            let _ = writeln!(file, "{error:#}");
                        }
                        Err(error) => {
                            let _ = writeln!(file, "{error}");
                        }
                        _ => (),
                    }
                }
            }
            let _ = writeln!(
                out,
                "{}",
                json!({"success":false,"message":"Adapter demonstration did not finish. Inspect its owner-only diagnostic and saved public stage records; reconcile any broadcast intent before retrying. Existing wallets and receipts were preserved."})
            );
            std::process::exit(1)
        }
    }
}
