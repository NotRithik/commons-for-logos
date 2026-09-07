use anyhow::{Context, Result, bail, ensure};
use astra_logos_testnet_primitives::*;
use borsh::BorshDeserialize;
use key_protocol::key_management::group_key_holder::GroupKeyHolder;
use lee::{
    AccountId, privacy_preserving_transaction::circuit::ProgramWithDependencies, program::Program,
};
use lee_core::program::ProgramId;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
use wallet::{
    AccountIdentity, WalletCore,
    config::{SequencerConnectionData, WalletConfig},
};
// This runner uses only testnet or a private loopback sequencer. It never starts
// model agents, calls a hosted prover, imports an existing user's wallet, or moves
// real funds. Witness-bearing operations stay inside the local proving boundary.

struct WalletLock(fs::File);
impl WalletLock {
    fn take(dir: &Path) -> Result<Self> {
        use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(dir.join(".astra-cli.lock"))?;
        // SAFETY: file holds a valid descriptor throughout this guard's lifetime.
        ensure!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "This test wallet already has an active CLI or integration writer"
        );
        Ok(Self(file))
    }
}
impl Drop for WalletLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        // SAFETY: the descriptor belongs to this guard and has not been closed.
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

fn save(path: &Path, value: &Value) -> Result<()> {
    let tmp = path.with_extension("json.new");
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}
fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    out.write_all(bytes)?;
    out.sync_all()?;
    Ok(())
}
fn event(dir: &Path, data: Value) -> Result<()> {
    println!("{}", data);
    let mut f = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(dir.join("evidence.jsonl"))?;
    writeln!(f, "{}", data)?;
    Ok(())
}
async fn open(dir: &Path) -> Result<WalletCore> {
    WalletCore::new_update_chain(
        dir.join("config.json"),
        dir.join("storage.json"),
        dir.join("statistics.json"),
        None,
    )
    .await
}
fn id(plan: &Value, name: &str) -> Result<AccountId> {
    plan[name]
        .as_str()
        .context("missing account ID")?
        .parse()
        .map_err(Into::into)
}
fn program(dir: &Path, name: &str, elf: &Path) -> Result<ProgramWithDependencies> {
    let plan: Value = serde_json::from_slice(&fs::read(dir.join("programs.json"))?)?;
    let program = Program::new(fs::read(elf)?.into())?;
    let expected: ProgramId = serde_json::from_value(plan[name].clone())?;
    ensure!(
        program.id() == expected,
        "program image changed from deployed value"
    );
    Ok(program.into())
}
async fn public_call<T: Serialize>(
    w: &mut WalletCore,
    dir: &Path,
    label: &str,
    account: AccountId,
    program: ProgramId,
    instruction: T,
) -> Result<()> {
    let start = Instant::now();
    let identity = if label == "execute" {
        AccountIdentity::PublicNoSign(account)
    } else {
        AccountIdentity::Public(account)
    };
    let hash = w
        .send_pub_tx(
            vec![identity],
            Program::serialize_instruction(instruction)?,
            program,
        )
        .await?;
    let (_, block) = w.poll_transaction(hash).await?;
    event(
        dir,
        json!({"stage":label,"tx_hash":hash.to_string(),"block_id":block,"seconds":start.elapsed().as_secs_f64(),"private":false}),
    )?;
    w.sync_to_latest_block().await?;
    w.store_persistent_data()?;
    Ok(())
}
fn private_account_snapshot(
    w: &WalletCore,
    dir: &Path,
    account: AccountId,
) -> Result<lee_core::account::Account> {
    if let Some(value) = w.get_account_private(account) {
        return Ok(value);
    }
    // The pinned v0.2.4 convenience getter only sees key-tree accounts, not shared
    // private accounts. Read our own just-synced wallet snapshot rather than silently
    // substituting a default account for a previously initialized member.
    let storage = wallet::storage::Storage::from_path(&dir.join("storage.json"))?;
    storage
        .key_chain()
        .shared_private_account(account)
        .map(|entry| entry.account.clone())
        .context("private member is absent from both regular and shared wallet state")
}
fn guest_preflight<T: Serialize>(
    dir: &Path,
    label: &str,
    program: &Program,
    accounts: &[lee_core::account::AccountWithMetadata],
    instruction: &T,
) -> Result<()> {
    use risc0_zkvm::{ExecutorEnv, default_executor};
    let words = Program::serialize_instruction(instruction)?;
    let mut builder = ExecutorEnv::builder();
    builder.session_limit(Some(32 * 1024 * 1024));
    builder
        .write(&program.id())?
        .write(&Option::<ProgramId>::None)?
        .write(&accounts)?
        .write(&words)?;
    let session = default_executor().execute(builder.build()?, program.elf())?;
    let out: lee_core::program::ProgramOutput = session.journal.decode()?;
    ensure!(
        out.pre_states == accounts,
        "guest pre-state binding mismatch"
    );
    lee_core::program::validate_execution(accounts, &out.post_states, program.id())?;
    // Only aggregate cycle counts are emitted. The journal contains private witness
    // data and is intentionally never written to disk, returned to UI, or logged.
    let cycles: u64 = session
        .segments
        .iter()
        .map(|segment| segment.cycles as u64)
        .sum();
    event(
        dir,
        json!({"stage": label, "status": "guest_preflight_passed", "guest_user_cycles": cycles, "proof": false}),
    )?;
    Ok(())
}
async fn private_call<T: Serialize>(
    w: &mut WalletCore,
    dir: &Path,
    label: &str,
    state: AccountId,
    witness: &MemberWitness,
    program: &ProgramWithDependencies,
    instruction: T,
) -> Result<()> {
    ensure!(
        std::env::var("RISC0_DEV_MODE").as_deref() == Ok("0"),
        "real proof mode required"
    );
    ensure!(
        std::env::var("RISC0_PROVER").as_deref() == Ok("ipc"),
        "explicit local IPC prover required; hosted proving is disabled"
    );
    let member = w
        .resolve_private_account(witness.leaf.account_id)
        .context("member private key missing")?;
    w.store_persistent_data()?;
    let pre_states = vec![
        lee_core::account::AccountWithMetadata {
            account: w.get_account_public(state).await?,
            is_authorized: false,
            account_id: state,
        },
        lee_core::account::AccountWithMetadata {
            account: private_account_snapshot(w, dir, witness.leaf.account_id)?,
            is_authorized: true,
            account_id: witness.leaf.account_id,
        },
    ];
    guest_preflight(dir, label, &program.program, &pre_states, &instruction)?;
    let start = Instant::now();
    event(
        dir,
        json!({"stage":label,"status":"proving","RISC0_DEV_MODE":"0"}),
    )?;
    let (hash, _secrets) = w
        .send_privacy_preserving_tx(
            vec![AccountIdentity::PublicNoSign(state), member],
            Program::serialize_instruction(instruction)?,
            program,
        )
        .await?;
    let (_, block) = w.poll_transaction(hash).await?;
    event(
        dir,
        json!({"stage":label,"status":"confirmed","tx_hash":hash.to_string(),"block_id":block,"seconds":start.elapsed().as_secs_f64(),"private":true}),
    )?;
    w.sync_to_latest_block().await?;
    w.store_persistent_data()?;
    Ok(())
}
async fn witnesses(
    w: &mut WalletCore,
    dir: &Path,
    name: &str,
    size: usize,
    program: ProgramId,
    state: AccountId,
) -> Result<Vec<MemberWitness>> {
    use lee_core::account::Account;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use wallet::storage::{Storage, key_chain::SharedAccountEntry};
    let dest = dir.join(format!("{name}-witnesses.bin"));
    let staged = dir.join(format!(".{name}-witnesses.staged.bin"));
    if dest.exists() {
        let list = Vec::<MemberWitness>::try_from_slice(&fs::read(dest)?)?;
        ensure!(list.len() == size, "existing witness count differs");
        return Ok(list);
    }
    // A staged witness is only recoverable if the matching wallet insertion landed.
    if staged.exists() {
        let list = Vec::<MemberWitness>::try_from_slice(&fs::read(&staged)?)?;
        ensure!(
            list.len() == size
                && list
                    .iter()
                    .all(|x| w.resolve_private_account(x.leaf.account_id).is_some()),
            "incomplete member checkpoint: wallet and staged witness disagree"
        );
        fs::rename(&staged, &dest)?;
        return Ok(list);
    }
    let birth_block = w.get_last_block_id().await?;
    w.store_persistent_data()?;
    let mut storage = Storage::from_path(&dir.join("storage.json"))?;
    let original_cursor = storage.last_synced_block();
    let scope = context(program, state);
    let mut result = Vec::new();
    for n in 0..size {
        // Only freshly generated, OS-random group secrets are permitted in this path.
        // Existing/imported keys MUST use upstream historical catch-up instead.
        let holder = GroupKeyHolder::new();
        let ident = (n as u128) + 1;
        let keys = holder.derive_regular_shared_account_keys_from_identifier(ident);
        let npk = keys.generate_nullifier_public_key();
        let vpk = keys.generate_viewing_public_key();
        let member = AccountId::from((&npk, &vpk, ident));
        let label = wallet::account::Label::new(format!("{name}-member-{n}"));
        ensure!(
            storage.key_chain().group_key_holder(&label).is_none(),
            "member label already exists; refusing replacement"
        );
        ensure!(
            storage.key_chain().shared_private_account(member).is_none(),
            "member already exists; refusing replacement"
        );
        storage
            .key_chain_mut()
            .insert_group_key_holder(label.clone(), holder);
        storage.key_chain_mut().insert_shared_private_account(
            member,
            SharedAccountEntry {
                group_label: label,
                identifier: ident,
                pda_seed: None,
                authority_program_id: None,
                account: Account::default(),
            },
        );
        let secret = keys.nullifier_secret_key;
        result.push(MemberWitness {
            leaf: MemberLeaf {
                account_id: member,
                salt: hash_parts(b"astra/demo/salt", &[&secret]),
                entitlement: 1,
            },
            nullifier_secret_key: secret,
            viewing_public_key: ViewingPublicKeyBytes::from_viewing_public_key(&vpk),
            identifier: ident,
            leaf_index: n as u32,
            siblings: Vec::new(),
        });
    }
    // Existing history is not skipped and the global sync cursor is NOT advanced.
    // Fresh keys did not exist before this call; re-scanning pre-birth history for
    // every newly generated member is unnecessary. All later updates are still synced.
    ensure!(
        storage.last_synced_block() == original_cursor,
        "cursor must never change during fresh key insertion"
    );
    let leaves: Vec<_> = result.iter().map(|x| x.leaf.commitment(&scope)).collect();
    for (i, x) in result.iter_mut().enumerate() {
        x.siblings = merkle_proof(&leaves, i)?.1;
    }
    private_write(&staged, &borsh::to_vec(&result)?)?;
    let next = dir.join(".storage-fresh-keys.json");
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&next)?;
    drop(file);
    storage.save_to_path(&next)?;
    fs::set_permissions(&next, fs::Permissions::from_mode(0o600))?;
    fs::rename(&next, dir.join("storage.json"))?;
    *w = open(dir).await?;
    ensure!(
        result
            .iter()
            .all(|x| w.resolve_private_account(x.leaf.account_id).is_some()),
        "fresh member wallet insertion verification failed"
    );
    fs::rename(&staged, &dest)?;
    event(
        dir,
        json!({"stage":"fresh_members_created","group":name,"count":size,"born_after_block":birth_block,"sync_cursor_unchanged":original_cursor,"historical_proofs_skipped":false}),
    )?;
    Ok(result)
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().context("expected mode; run with --help")?;
    if mode == "--help" || mode == "help" {
        println!(
            "Astra Logos E2E runner (testnet only)\n\
prepare-local DIR (offline keys for a fresh local genesis)\n\
prepare DIR [http://127.0.0.1:34341|https://testnet.lez.logos.co]\n\
deploy DIR ARTIFACT_DIRECTORY\n\
claims DIR ALLOWLIST_ARTIFACT [MAX_CLAIMS=20]\n\
claims-smoke DIR ALLOWLIST_ARTIFACT (one real claim in each distribution)\n\
threshold DIR THRESHOLD_ARTIFACT\n\
verify DIR [minimum_claims=20]\n\
GUI membership setup: setup-gui DIR [allowlist|threshold]\n\
Member-only wallet: member-profile DIR ORDINAL [threshold|allowlist]\n\
Proofs require RISC0_DEV_MODE=0, RISC0_PROVER=ipc; public use additionally requires ASTRA_ALLOW_PUBLIC_TESTNET=1."
        );
        return Ok(());
    }
    let dir = PathBuf::from(args.next().context("expected isolated runtime directory")?);
    fs::create_dir_all(&dir)?;
    ensure!(
        !fs::symlink_metadata(&dir)?.file_type().is_symlink(),
        "test wallet directory must not be a symlink"
    );
    let _wallet_lock = WalletLock::take(&dir)?;
    if mode == "image-id" {
        for filename in args {
            let bytes = fs::read(&filename)?;
            let p = Program::new(bytes.into())?;
            println!("{}", json!({"path":filename,"image_id":p.id()}));
        }
        return Ok(());
    }
    if mode == "probe" {
        use sequencer_service_rpc::{RpcClient as _, SequencerClientBuilder};
        let client = SequencerClientBuilder::default()
            .request_timeout(std::time::Duration::from_secs(15))
            .build("https://testnet.lez.logos.co")?;
        println!(
            "Read-only public testnet probe: {:?}",
            client.get_last_block_id().await
        );
        return Ok(());
    }
    if mode == "prepare-local" {
        // Build an entirely new wallet before a local sequencer exists, so its
        // genesis can fund only these newly generated public fixture accounts.
        ensure!(
            !dir.join("storage.json").exists(),
            "refusing to replace an existing test wallet"
        );
        let (mut storage, _mnemonic) =
            wallet::storage::Storage::new("fresh-local-test-fixture-not-encrypted")?;
        let mut plan = json!({});
        for name in [
            "payer",
            "distribution_a",
            "distribution_b",
            "threshold_group",
        ] {
            plan[name] = json!(
                storage
                    .key_chain_mut()
                    .generate_new_public_transaction_private_key(None)
                    .0
                    .to_string()
            );
        }
        let mut config = WalletConfig::default();
        config.sequencers = vec![SequencerConnectionData {
            sequencer_addr: "http://127.0.0.1:34341".parse()?,
            basic_auth: None,
        }];
        config.multi_sequencer_client_config.calibration_limit = 2;
        config.seq_tx_poll_max_blocks = 180;
        config.seq_poll_timeout = std::time::Duration::from_millis(500);
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        private_write(&dir.join("storage.json"), b"{}")?;
        storage.save_to_path(&dir.join("storage.json"))?;
        fs::set_permissions(dir.join("storage.json"), fs::Permissions::from_mode(0o600))?;
        save(&dir.join("config.json"), &serde_json::to_value(config)?)?;
        save(&dir.join("statistics.json"), &json!({}))?;
        save(&dir.join("public-plan.json"), &plan)?;
        fs::write(
            dir.join(".astra-logos-testnet-wallet"),
            b"LOCAL ONLY fresh fixture. No mainnet value. Plaintext owner-only wallet storage.\n",
        )?;
        event(
            &dir,
            json!({"stage":"prepared_offline","test_only":true,"network":"loopback","public_accounts":plan,"no_network_request":true}),
        )?;
        return Ok(());
    }
    if mode == "prepare" {
        ensure!(
            !dir.join("storage.json").exists(),
            "refusing to replace an existing test wallet"
        );
        let mut config = WalletConfig::default();
        let endpoint = args
            .next()
            .unwrap_or_else(|| "http://127.0.0.1:34341".into());
        ensure!(
            endpoint == "http://127.0.0.1:34341" || endpoint == "https://testnet.lez.logos.co",
            "unsupported endpoint"
        );
        config.sequencers = vec![SequencerConnectionData {
            sequencer_addr: endpoint.parse()?,
            basic_auth: None,
        }];
        config.multi_sequencer_client_config.calibration_limit = 2;
        config.seq_tx_poll_max_blocks = 180;
        config.seq_poll_timeout = std::time::Duration::from_millis(500);
        fs::write(dir.join("config.json"), serde_json::to_vec_pretty(&config)?)?;
        let (mut w, _mnemonic) = WalletCore::new_init_storage(
            dir.join("config.json"),
            dir.join("storage.json"),
            dir.join("statistics.json"),
            None,
            "astra-local-fixture-only-never-use-for-money",
        )
        .await?;
        // Only this brand-new wallet may use a birthday checkpoint. It has no
        // pre-existing keys or receipts, so no relevant earlier history can exist.
        let birth_block = w.get_last_block_id().await?;
        let mut plan = json!({});
        for name in [
            "payer",
            "distribution_a",
            "distribution_b",
            "threshold_group",
        ] {
            plan[name] = json!(w.create_new_account_public(None).0.to_string());
        }
        w.store_persistent_data()?;
        let mut fresh = wallet::storage::Storage::from_path(&dir.join("storage.json"))?;
        ensure!(
            fresh.last_synced_block() == 0,
            "fresh wallet already has history; refuse birthday shortcut"
        );
        fresh.set_last_synced_block(birth_block);
        fresh.save_to_path(&dir.join("storage.json"))?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir.join("storage.json"), fs::Permissions::from_mode(0o600))?;
        fs::write(
            dir.join(".astra-logos-testnet-wallet"),
            b"Fresh testnet-only fixture. Never fund with real value.\n",
        )?;
        save(&dir.join("public-plan.json"), &plan)?;
        event(
            &dir,
            json!({"stage":"prepared","test_only":true,"public_accounts":plan,"wallet_birthday_block":birth_block,"new_wallet_only":true}),
        )?;
        return Ok(());
    }
    if mode == "inspect" {
        for (name, path) in [
            (
                "privacy_preserving_circuit",
                dir.join("artifacts/lee/privacy_preserving_circuit/privacy_preserving_circuit.bin"),
            ),
            (
                "authenticated_transfer",
                dir.join("artifacts/lez/programs/authenticated_transfer.bin"),
            ),
            ("token", dir.join("artifacts/lez/programs/token.bin")),
        ] {
            let checked = Program::new(fs::read(path)?.into())?;
            println!("{}", json!({"program":name,"image_id":checked.id()}));
        }
        return Ok(());
    }
    if mode == "pack" {
        let raw = PathBuf::from(args.next().context("expected raw guest directory")?);
        for file in ["astra_allowlist", "astra_threshold"] {
            let user = fs::read(raw.join(file))?;
            let packed =
                risc0_binfmt::ProgramBinary::new(&user, risc0_zkos_v1compat::V1COMPAT_ELF).encode();
            let checked = Program::new(packed.clone().into())?;
            fs::write(dir.join(file), &packed)?;
            println!(
                "{}",
                json!({"name":file,"bytes":packed.len(),"image_id":checked.id()})
            );
        }
        return Ok(());
    }
    let mut w = open(&dir).await?;
    let endpoint = w.helm_url();
    let is_public_testnet = endpoint.scheme() == "https"
        && endpoint.host_str() == Some("testnet.lez.logos.co")
        && endpoint.port_or_known_default() == Some(443);
    ensure!(
        endpoint.as_str() == "http://127.0.0.1:34341/" || is_public_testnet,
        "unsupported endpoint"
    );
    if is_public_testnet {
        ensure!(
            std::env::var("ASTRA_ALLOW_PUBLIC_TESTNET").as_deref() == Ok("1"),
            "public TESTNET flag required"
        );
        let ids = w.get_program_ids().await?;
        ensure!(
            ids.get("privacy_preserving_circuit")
                == Some(&[
                    1334328888, 3910590567, 1244219104, 3671232111, 3138827701, 405554639,
                    4064616947, 1864368340
                ]),
            "testnet protocol changed: refuse mismatched proof ABI"
        );
    }

    let plan: Value = serde_json::from_slice(&fs::read(dir.join("public-plan.json"))?)?;
    if mode == "verify" {
        let minimum: usize = args.next().unwrap_or_else(|| "20".into()).parse()?;
        ensure!(minimum <= 20, "at most 20 demo claims expected");
        let programs: Value = serde_json::from_slice(&fs::read(dir.join("programs.json"))?)?;
        let allowlist_pid: ProgramId = serde_json::from_value(programs["allowlist"].clone())?;
        let threshold_pid: ProgramId = serde_json::from_value(programs["threshold"].clone())?;
        let mut total = 0usize;
        let mut distributions = Vec::new();
        for name in ["distribution_a", "distribution_b"] {
            let state_id = id(&plan, name)?;
            let account = w.get_account_public(state_id).await?;
            if account.data.is_empty() {
                continue;
            }
            ensure!(
                account.program_owner == allowlist_pid,
                "distribution program owner mismatch"
            );
            let state = Distribution::try_from_slice(&account.data)?;
            let unique: std::collections::HashSet<_> = state.claims.iter().collect();
            ensure!(
                unique.len() == state.claims.len(),
                "duplicate on-chain nullifier"
            );
            total += unique.len();
            distributions.push(json!({"state_account":state_id.to_string(), "confirmed_claims":unique.len(), "member_count":state.member_count}));
        }
        ensure!(
            total >= minimum,
            "not enough confirmed on-chain claims: {total} < {minimum}"
        );
        if minimum == 20 {
            ensure!(
                distributions.len() == 2,
                "two distinct distributions are required"
            );
        }
        let group_id = id(&plan, "threshold_group")?;
        let account = w.get_account_public(group_id).await?;
        ensure!(
            account.program_owner == threshold_pid,
            "threshold program owner mismatch"
        );
        let group = Group::try_from_slice(&account.data)?;
        let proposal = group.proposal.context("no on-chain proposal")?;
        ensure!(
            proposal.executed && group.value == 42 && proposal.approvals.len() >= 2,
            "threshold execution is not confirmed"
        );
        let block = w.get_last_block_id().await?;
        event(
            &dir,
            json!({"stage":"independent_state_verification", "network":endpoint.as_str(), "block_id":block,
            "confirmed_unique_claims":total,"distributions":distributions,"threshold_group":group_id.to_string(),
            "threshold_executed":true,"value":group.value,"minimum_claims_required":minimum}),
        )?;
        return Ok(());
    }
    if mode == "member-profile" {
        let index: usize = args.next().context("member ordinal required")?.parse()?;
        let family = args.next().unwrap_or_else(|| "threshold".into());
        ensure!(
            family == "threshold" || family == "allowlist",
            "unknown member-profile family"
        );
        let name = if family == "threshold" {
            "threshold"
        } else {
            "distribution_a"
        };
        let list: Vec<MemberWitness> =
            borsh::from_slice(&fs::read(dir.join(format!("{name}-witnesses.bin")))?)?;
        let witness = list
            .get(index)
            .context("member ordinal outside eligibility set")?;
        w.sync_to_latest_block().await?;
        w.store_persistent_data()?;
        let original = wallet::storage::Storage::from_path(&dir.join("storage.json"))?;
        let entry = original
            .key_chain()
            .shared_private_account(witness.leaf.account_id)
            .context("shared member entry missing")?
            .clone();
        let holder = original
            .key_chain()
            .group_key_holder(&entry.group_label)
            .context("member group key missing")?
            .clone();
        let (mut isolated, _mnemonic) =
            wallet::storage::Storage::new("testnet-fixture-not-encrypted-by-upstream")?;
        isolated
            .key_chain_mut()
            .insert_group_key_holder(entry.group_label.clone(), holder);
        isolated
            .key_chain_mut()
            .insert_shared_private_account(witness.leaf.account_id, entry);
        // Copy the exact verified sync checkpoint associated with the imported
        // account state. This never skips new or unverified history.
        isolated.set_last_synced_block(original.last_synced_block());
        ensure!(
            isolated.key_chain().group_key_holders_iter().count() == 1,
            "member profile must have exactly one group key"
        );
        ensure!(
            isolated.key_chain().shared_private_accounts_iter().count() == 1,
            "member profile must have exactly one private member"
        );
        let parent = dir.join("member-profiles");
        fs::create_dir_all(&parent)?;
        let destination = parent.join(format!("{family}-{index}"));
        ensure!(
            !destination.exists(),
            "refusing to overwrite an existing member profile"
        );
        fs::create_dir(&destination)?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o700))?;
        private_write(&destination.join("storage.json"), b"{}")?;
        isolated.save_to_path(&destination.join("storage.json"))?;
        fs::set_permissions(
            destination.join("storage.json"),
            fs::Permissions::from_mode(0o600),
        )?;
        for filename in ["config.json", "programs.json", "public-plan.json"] {
            fs::copy(dir.join(filename), destination.join(filename))?;
        }
        fs::write(
            destination.join(".astra-logos-testnet-wallet"),
            b"Single-member TESTNET fixture. Plaintext owner-only key storage; no real funds.\n",
        )?;
        fs::create_dir(destination.join("artifacts"))?;
        for artifact in ["astra_allowlist", "astra_threshold"] {
            fs::copy(
                dir.join("artifacts").join(artifact),
                destination.join("artifacts").join(artifact),
            )?;
        }
        private_write(
            &destination.join("member-witness.bin"),
            &borsh::to_vec(witness)?,
        )?;
        fs::copy(
            dir.join(format!("{family}-gui-plan.json")),
            destination.join(format!("{family}-gui-plan.json")),
        )?;
        event(
            &dir,
            json!({"stage":"isolated_member_profile_created","family":family,"member_ordinal":index,
            "private_keys_in_profile":1,"other_member_keys_copied":false,"coordinator_public_signing_keys_copied":false,
            "sync_checkpoint":isolated.last_synced_block(),"secret_material_printed":false}),
        )?;
        return Ok(());
    }
    if mode == "setup-gui" {
        let family = args.next().unwrap_or_else(|| "threshold".into());
        ensure!(
            family == "threshold" || family == "allowlist",
            "unknown GUI workflow"
        );
        let programs: Value = serde_json::from_slice(&fs::read(dir.join("programs.json"))?)?;
        let pid: ProgramId = serde_json::from_value(programs[&family].clone())?;
        let name = if family == "threshold" {
            "threshold"
        } else {
            "distribution_a"
        };
        let state_id = id(
            &plan,
            if family == "threshold" {
                "threshold_group"
            } else {
                "distribution_a"
            },
        )?;
        let count = if family == "threshold" { 3 } else { 10 };
        let members = witnesses(&mut w, &dir, name, count, pid, state_id).await?;
        let scope = context(pid, state_id);
        let leaves: Vec<_> = members.iter().map(|m| m.leaf.commitment(&scope)).collect();
        let destination = dir.join("gui-witnesses");
        fs::create_dir_all(&destination)?;
        for (index, member) in members.iter().enumerate() {
            let path = destination.join(format!("{family}-member-{index}.bin"));
            if !path.exists() {
                private_write(&path, &borsh::to_vec(member)?)?;
            }
        }
        save(
            &dir.join(format!("{family}-gui-plan.json")),
            &json!({"family":family,
            "state_account":hex::encode(state_id.as_ref()),"root":hex::encode(merkle_proof(&leaves,0)?.0),
            "member_count":count,"threshold":if family=="threshold" {Some(2)} else {None},
            "initial_value":"7","next_value":"42","witness_directory":"gui-witnesses"}),
        )?;
        event(
            &dir,
            json!({"stage":"gui_membership_prepared","family":family,"member_count":count,
            "private_witnesses_not_printed":true,"no_transaction_submitted":true}),
        )?;
        return Ok(());
    }
    if mode == "deploy" {
        let elfdir = PathBuf::from(args.next().context("expected guest artifact directory")?);
        let mut deployed: Value = if dir.join("programs.json").exists() {
            serde_json::from_slice(&fs::read(dir.join("programs.json"))?)?
        } else {
            json!({})
        };
        for (name, file) in [
            ("allowlist", "astra_allowlist"),
            ("threshold", "astra_threshold"),
        ] {
            if deployed.get(name).is_some() {
                continue;
            }
            let bytes = fs::read(elfdir.join(file))?;
            let image = Program::new(bytes.clone().into())?.id();
            w.store_persistent_data()?;
            let start = Instant::now();
            let hash = w.send_program_deployment_transaction(bytes).await?;
            let (_, block) = w.poll_transaction(hash).await?;
            deployed[name] = json!(image);
            save(&dir.join("programs.json"), &deployed)?;
            event(
                &dir,
                json!({"stage":"deployed","program":name,"tx_hash":hash.to_string(),"block_id":block,"image_id":image,"seconds":start.elapsed().as_secs_f64()}),
            )?;
        }
        return Ok(());
    }
    if mode == "claims" || mode == "claims-a" || mode == "claims-b" || mode == "claims-smoke" {
        let elf = PathBuf::from(args.next().context("expected allowlist artifact")?);
        let limit: usize = args.next().unwrap_or_else(|| "20".into()).parse()?;
        ensure!(limit <= 20, "at most 20 demo identities are available");
        let per_distribution = if mode == "claims-smoke" { 1 } else { 10 };
        let mut confirmed_by_distribution = Vec::new();
        let prog = program(&dir, "allowlist", &elf)?;
        for (d, name) in ["distribution_a", "distribution_b"].iter().enumerate() {
            if (mode == "claims-a" && d != 0) || (mode == "claims-b" && d != 1) {
                continue;
            }
            let state = id(&plan, name)?;
            let list = witnesses(&mut w, &dir, name, 10, prog.program.id(), state).await?;
            let scope = context(prog.program.id(), state);
            let leaves: Vec<_> = list.iter().map(|x| x.leaf.commitment(&scope)).collect();
            if w.get_account_public(state).await?.data.is_empty() {
                public_call(
                    &mut w,
                    &dir,
                    &format!("create_{name}"),
                    state,
                    prog.program.id(),
                    DistributionInstruction::Create {
                        root: merkle_proof(&leaves, 0)?.0,
                        member_count: 10,
                    },
                )
                .await?;
            }
            for (i, witness) in list.iter().enumerate().take(per_distribution) {
                if mode != "claims-smoke" && d * 10 + i >= limit {
                    break;
                }
                let current =
                    Distribution::try_from_slice(&w.get_account_public(state).await?.data)?;
                if current.claims.len() > i {
                    continue;
                }
                private_call(
                    &mut w,
                    &dir,
                    &format!("claim_{name}_{i}"),
                    state,
                    witness,
                    &prog,
                    DistributionInstruction::Claim {
                        witness: witness.clone(),
                    },
                )
                .await?;
                let current =
                    Distribution::try_from_slice(&w.get_account_public(state).await?.data)?;
                ensure!(
                    current.claims.len() == i + 1,
                    "confirmed claim count mismatch"
                );
            }
            let observed = Distribution::try_from_slice(&w.get_account_public(state).await?.data)?;
            let distinct: std::collections::HashSet<_> = observed.claims.iter().collect();
            ensure!(
                distinct.len() == observed.claims.len(),
                "duplicate persisted nullifier"
            );
            confirmed_by_distribution.push(json!({"name":name,"state_account":state.to_string(),"unique_claims":distinct.len()}));
        }
        let total: usize = confirmed_by_distribution
            .iter()
            .map(|r| r["unique_claims"].as_u64().unwrap_or(0) as usize)
            .sum();
        let required = if mode == "claims-smoke" {
            2
        } else if mode == "claims" {
            20
        } else {
            10
        };
        event(
            &dir,
            json!({"stage":if total >= required {"claims_complete"} else {"claims_partial"},"mode":mode,
                "distributions":confirmed_by_distribution,"unique_private_claims":total,
                "required_for_this_mode":required,"network":w.helm_url().as_str()}),
        )?;
        return Ok(());
    }
    if mode == "threshold" {
        let elf = PathBuf::from(args.next().context("expected threshold artifact")?);
        let prog = program(&dir, "threshold", &elf)?;
        let state = id(&plan, "threshold_group")?;
        let list = witnesses(&mut w, &dir, "threshold", 3, prog.program.id(), state).await?;
        let scope = context(prog.program.id(), state);
        let leaves: Vec<_> = list.iter().map(|x| x.leaf.commitment(&scope)).collect();
        if w.get_account_public(state).await?.data.is_empty() {
            public_call(
                &mut w,
                &dir,
                "create_threshold",
                state,
                prog.program.id(),
                GroupInstruction::Create {
                    root: merkle_proof(&leaves, 0)?.0,
                    member_count: 3,
                    threshold: 2,
                    initial_value: 7,
                },
            )
            .await?;
        }
        let group = Group::try_from_slice(&w.get_account_public(state).await?.data)?;
        if group.proposal.is_none() {
            private_call(
                &mut w,
                &dir,
                "propose",
                state,
                &list[0],
                &prog,
                GroupInstruction::Propose {
                    witness: list[0].clone(),
                    next_value: 42,
                },
            )
            .await?;
        }
        for (i, witness) in list.iter().enumerate().take(2) {
            let group = Group::try_from_slice(&w.get_account_public(state).await?.data)?;
            if group.proposal.as_ref().unwrap().approvals.len() > i {
                continue;
            }
            private_call(
                &mut w,
                &dir,
                &format!("approve_{i}"),
                state,
                witness,
                &prog,
                GroupInstruction::Approve {
                    witness: witness.clone(),
                },
            )
            .await?;
        }
        let group = Group::try_from_slice(&w.get_account_public(state).await?.data)?;
        if !group.proposal.as_ref().unwrap().executed {
            public_call(
                &mut w,
                &dir,
                "execute",
                state,
                prog.program.id(),
                GroupInstruction::Execute,
            )
            .await?;
        }
        let group = Group::try_from_slice(&w.get_account_public(state).await?.data)?;
        ensure!(
            group.value == 42 && group.proposal.unwrap().executed,
            "threshold not executed"
        );
        event(
            &dir,
            json!({"stage":"threshold_complete","threshold":"2 of 3","parameter_value":42,"network":w.helm_url().as_str()}),
        )?;
        return Ok(());
    }
    bail!("unsupported mode");
}
