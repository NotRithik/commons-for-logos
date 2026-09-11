//! Local governance onboarding. Enrollment and invitation documents carry public
//! membership material only. A member's private key never leaves their profile.
//! These verbs do NOT publish, propose, approve, execute, fund or claim a faucet.
use crate::{
    WalletLock,
    validation::{TESTNET_URL, account, hash, integer, read_inside},
};
use anyhow::{Context, Result, ensure};
use commons_logos_testnet_primitives::{
    MemberLeaf, MemberWitness, ViewingPublicKeyBytes, context, hash_parts, merkle_proof,
};
use lee::program::Program;
use lee_core::{PrivateAccountKind, program::ProgramId};
use sequencer_service_rpc::{RpcClient as _, SequencerClientBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};
use wallet::storage::Storage;

const LIMIT: u64 = 1024 * 1024;
const MAX_MEMBERS: usize = 256;

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WorkspaceKind {
    #[default]
    Threshold,
    Allowlist,
}
impl WorkspaceKind {
    fn family(self) -> &'static str {
        match self {
            Self::Threshold => "threshold",
            Self::Allowlist => "allowlist",
        }
    }
    fn value_type(self) -> &'static str {
        match self {
            Self::Threshold => "i64",
            Self::Allowlist => "membership",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    schema_version: u32,
    home: PathBuf,
    profile: String,
    arguments: Value,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Enrollment {
    schema_version: u32,
    label: String,
    account_id: String,
    salt: String,
}
impl Enrollment {
    fn leaf(&self) -> Result<MemberLeaf> {
        ensure!(self.schema_version == 1, "Unsupported enrollment version.");
        label(&self.label)?;
        Ok(MemberLeaf {
            account_id: account(&self.account_id)?,
            salt: hash(&self.salt)?,
            entitlement: 1,
        })
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invitation {
    schema_version: u32,
    #[serde(default)]
    kind: WorkspaceKind,
    program_id: ProgramId,
    state_account: String,
    root: String,
    title: String,
    field_name: String,
    value_type: String,
    initial_value: String,
    member_count: u32,
    threshold: u32,
    member: Enrollment,
    leaf_index: u32,
    siblings: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    #[serde(default)]
    kind: WorkspaceKind,
    state_account: String,
    root: String,
    title: String,
    field_name: String,
    value_type: String,
    initial_value: String,
    member_count: u32,
    threshold: u32,
    creator: bool,
    invitations: Vec<Invitation>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    schema_version: u32,
    id: String,
    label: String,
    enrollment: Enrollment,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateIdentity {
    label: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparePolicy {
    title: String,
    field_name: String,
    initial_value: String,
    threshold: u32,
    members: Vec<Enrollment>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrepareList {
    title: String,
    members: Vec<Enrollment>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JoinPolicy {
    invitation: Invitation,
}

fn label(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty()
            && value.chars().count() <= 80
            && !value.chars().any(char::is_control),
        "Use a short label without control characters."
    );
    Ok(())
}
fn nonce() -> Result<String> {
    let mut bytes = [0u8; 16];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(hex::encode(bytes))
}
fn checked_home(path: &Path) -> Result<PathBuf> {
    ensure!(
        path.is_absolute() && !path.components().any(|p| matches!(p, Component::ParentDir)),
        "Choose an absolute governance folder."
    );
    if !path.exists() {
        fs::create_dir_all(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink() && meta.uid() == unsafe { libc::geteuid() },
        "Governance folder must be a regular directory owned by you."
    );
    ensure!(
        meta.permissions().mode() & 0o077 == 0,
        "Governance folder must be private to its owner (mode 700)."
    );
    Ok(path.canonicalize()?)
}
fn checked_profile(home: &Path, id: &str) -> Result<PathBuf> {
    ensure!(
        id.len() == 32
            && id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "Choose a saved identity."
    );
    let root = home.join(id);
    let meta = fs::symlink_metadata(&root)?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "Identity folder is not a regular directory."
    );
    ensure!(
        root.canonicalize()?.parent() == Some(home),
        "Identity folder is outside governance home."
    );
    Ok(root)
}
fn load<T: serde::de::DeserializeOwned>(root: &Path, file: &str) -> Result<T> {
    Ok(serde_json::from_slice(&read_inside(
        root,
        &root.join(file),
        LIMIT,
        true,
    )?)?)
}
fn write_new(path: &Path, data: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}
fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if path.exists() {
        ensure!(
            !fs::symlink_metadata(path)?.file_type().is_symlink(),
            "Refusing a linked output file."
        );
    }
    let temp = path.with_extension(format!("{}.tmp", nonce()?));
    write_new(&temp, &serde_json::to_vec_pretty(value)?)?;
    fs::rename(temp, path)?;
    Ok(())
}
fn save_storage(root: &Path, storage: &Storage) -> Result<()> {
    let temp = root.join(format!(".storage-{}.json", nonce()?));
    write_new(&temp, b"{}")?;
    storage.save_to_path(&temp)?;
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
    fs::rename(temp, root.join("storage.json"))?;
    Ok(())
}
fn program_id(kind: WorkspaceKind) -> Result<ProgramId> {
    let manifest: Value = serde_json::from_str(include_str!("../../release/manifest.json"))?;
    Ok(serde_json::from_value(
        manifest["programs"][kind.family()]["image_id"].clone(),
    )?)
}
fn release_artifacts() -> Result<Vec<(String, Vec<u8>, ProgramId)>> {
    let dir = std::env::current_exe()?
        .parent()
        .context("Missing executable directory")?
        .join("artifacts");
    let manifest: Value = serde_json::from_str(include_str!("../../release/manifest.json"))?;
    let mut result = Vec::new();
    for family in ["threshold", "allowlist"] {
        let filename = format!("commons_{family}");
        let path = dir.join(&filename);
        if family == "allowlist" && !path.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).context("Install the matching program artifacts next to commons-logos-cli before creating an identity.")?;
        ensure!(
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() < 8 * 1024 * 1024,
            "Invalid release artifact."
        );
        let data = fs::read(path)?;
        let pid: ProgramId =
            serde_json::from_value(manifest["programs"][family]["image_id"].clone())?;
        ensure!(
            Program::new(data.clone().into())?.id() == pid,
            "Release artifact does not match the published program ID."
        );
        result.push((filename, data, pid));
    }
    Ok(result)
}
fn policy_file(state: &str) -> Result<String> {
    account(state)?;
    Ok(format!("policy-{}.json", hex::encode(hash(state)?)))
}
fn witness_file(state: &str) -> Result<String> {
    account(state)?;
    Ok(format!("member-{}.bin", hex::encode(hash(state)?)))
}
fn policies(root: &Path) -> Result<Vec<Policy>> {
    let mut names = Vec::new();
    for item in fs::read_dir(root)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with("policy-") && name.ends_with(".json") {
            names.push(name);
        }
    }
    ensure!(names.len() <= 256, "Too many policies in this identity.");
    names.sort();
    names.iter().map(|name| load(root, name)).collect()
}
fn catalog(home: &Path, selected: &str) -> Result<Value> {
    let mut profiles = Vec::new();
    for item in fs::read_dir(home)? {
        let item = item?;
        let id = item.file_name().to_string_lossy().into_owned();
        if id.len() != 32 || !item.file_type()?.is_dir() || item.file_type()?.is_symlink() {
            continue;
        }
        let root = checked_profile(home, &id)?;
        if !root.join("profile.json").exists() {
            continue;
        } // Incomplete creation is never presented as ready.
        let p: Profile = load(&root, "profile.json")?;
        ensure!(
            p.schema_version == 1 && p.id == id,
            "Identity metadata mismatch."
        );
        label(&p.label)?;
        // The enrollment DTO carries no private key or witness.
        profiles.push(json!({"id":id,"label":p.label,"enrollment":p.enrollment}));
    }
    ensure!(
        profiles.len() <= 64,
        "Too many identities. Use separate governance homes."
    );
    profiles.sort_by_key(|p| p["label"].as_str().unwrap_or("").to_owned());
    let mut result = json!({"success":true,"profiles":profiles,"selected_profile":selected,"policies":[],"transactions_submitted":0});
    if !selected.is_empty() {
        let root = checked_profile(home, selected)?;
        let p: Profile = load(&root, "profile.json")?;
        let mut rows = Vec::new();
        for policy in policies(&root)? {
            let mut v = serde_json::to_value(&policy)?;
            let witness = root.join(witness_file(&policy.state_account)?);
            v["witness_file"] =
                if witness.is_file() && !fs::symlink_metadata(&witness)?.file_type().is_symlink() {
                    json!(witness)
                } else {
                    Value::Null
                };
            rows.push(v);
        }
        result["profile_dir"] = json!(root);
        result["identity_label"] = json!(p.label);
        result["enrollment"] = serde_json::to_value(p.enrollment)?;
        result["policies"] = json!(rows);
    }
    Ok(result)
}
async fn create_identity(home: &Path, args: CreateIdentity) -> Result<String> {
    label(&args.label)?;
    let existing = catalog(home, "")?;
    let names = existing["profiles"]
        .as_array()
        .context("Identity catalog invalid")?;
    ensure!(
        !names.iter().any(|row| row["label"]
            .as_str()
            .is_some_and(|name| name.to_lowercase() == args.label.to_lowercase())),
        "An identity with that name already exists. Choose it from the identity list or use a different name."
    );
    ensure!(names.len() < 64, "Identity limit reached.");
    let artifacts = release_artifacts()?;
    let client = SequencerClientBuilder::default()
        .request_timeout(std::time::Duration::from_secs(20))
        .build(TESTNET_URL)?;
    let ids = client.get_program_ids().await?;
    ensure!(
        ids.get("privacy_preserving_circuit") == Some(&crate::validation::PROTOCOL_ID),
        "Testnet protocol changed; no new identity created."
    );
    // This checkpoint is for keys generated BELOW this read. Never use it for
    // an imported wallet, old keys or an existing private account.
    let birth = client.get_last_block_id().await?;
    let id = nonce()?;
    let root = home.join(&id);
    fs::create_dir(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let (mut storage, _mnemonic) =
        Storage::new("testnet-only-plaintext-profile-not-a-production-password")?;
    let member = storage
        .key_chain_mut()
        .generate_new_privacy_preserving_transaction_key_chain(None)
        .0;
    let found = storage
        .key_chain()
        .private_account(member)
        .context("Fresh member key unavailable")?;
    let salt = hash_parts(
        b"commons/enrollment/salt/v1",
        &[&found.key_chain.private_key_holder.nullifier_secret_key],
    );
    let enrollment = Enrollment {
        schema_version: 1,
        label: args.label.clone(),
        account_id: hex::encode(member.as_ref()),
        salt: hex::encode(salt),
    };
    ensure!(
        storage.last_synced_block() == 0,
        "Fresh profile unexpectedly has history."
    );
    storage.set_last_synced_block(birth);
    save_storage(&root, &storage)?;
    let config = json!({"sequencers":[{"sequencer_addr":TESTNET_URL}],"seq_poll_timeout":"500ms","seq_tx_poll_max_blocks":180,"seq_poll_max_retries":5,"seq_block_poll_max_amount":100,"multi_sequencer_client_config":{"distribution_limit":1,"calibration_limit":2}});
    write_new(
        &root.join("config.json"),
        &serde_json::to_vec_pretty(&config)?,
    )?;
    write_new(&root.join("statistics.json"), b"{}")?;
    write_new(
        &root.join(".commons-logos-testnet-wallet"),
        b"Testnet only. Owner-only plaintext keys. Never use for mainnet value.\n",
    )?;
    fs::create_dir(root.join("artifacts"))?;
    let mut program_ids = json!({});
    for (file, data, pid) in artifacts {
        write_new(&root.join("artifacts").join(&file), &data)?;
        program_ids[file.trim_start_matches("commons_")] = json!(pid);
    }
    write_new(
        &root.join("programs.json"),
        &serde_json::to_vec_pretty(&program_ids)?,
    )?;
    write_new(
        &root.join("enrollment.json"),
        &serde_json::to_vec_pretty(&enrollment)?,
    )?;
    // This metadata is the commit marker. A partially created profile is not
    // selected or overwritten on retry; its keys remain intact for inspection.
    write_new(
        &root.join("profile.json"),
        &serde_json::to_vec_pretty(&Profile {
            schema_version: 1,
            id: id.clone(),
            label: args.label,
            enrollment,
        })?,
    )?;
    Ok(id)
}
fn prepare_policy(
    home: &Path,
    profile: &str,
    args: PreparePolicy,
    kind: WorkspaceKind,
) -> Result<String> {
    let root = checked_profile(home, profile)?;
    let _lock = WalletLock::take(&root)?;
    label(&args.title)?;
    label(&args.field_name)?;
    integer(&args.initial_value)?;
    ensure!(
        (1..=MAX_MEMBERS).contains(&args.members.len())
            && args.threshold > 0
            && args.threshold as usize <= args.members.len(),
        "Choose between 1 and 256 members and a threshold between 1 and that count."
    );
    ensure!(policies(&root)?.len() < 256, "Policy limit reached.");
    let mut unique = BTreeSet::new();
    let mut leaves_raw = Vec::new();
    for member in &args.members {
        let leaf = member.leaf()?;
        ensure!(
            unique.insert(hex::encode(leaf.account_id.as_ref())),
            "The same member cannot occupy two seats."
        );
        leaves_raw.push(leaf);
    }
    let mut storage = Storage::from_path(&root.join("storage.json"))?;
    let state = storage
        .key_chain_mut()
        .generate_new_public_transaction_private_key(None)
        .0;
    let pid = program_id(kind)?;
    let scope = context(pid, state);
    let leaves: Vec<_> = leaves_raw.iter().map(|l| l.commitment(&scope)).collect();
    let (merkle_root, _) = merkle_proof(&leaves, 0)?;
    let state_hex = hex::encode(state.as_ref());
    let invitations: Vec<Invitation> = args
        .members
        .into_iter()
        .enumerate()
        .map(|(i, member)| -> Result<Invitation> {
            let (_, proof) = merkle_proof(&leaves, i)?;
            Ok(Invitation {
                schema_version: 1,
                kind,
                program_id: pid,
                state_account: state_hex.clone(),
                root: hex::encode(merkle_root),
                title: args.title.clone(),
                field_name: args.field_name.clone(),
                value_type: kind.value_type().into(),
                initial_value: args.initial_value.clone(),
                member_count: leaves.len() as u32,
                threshold: args.threshold,
                member,
                leaf_index: i as u32,
                siblings: proof.iter().map(hex::encode).collect(),
            })
        })
        .collect::<Result<_>>()?;
    // Persist ownership of the fresh public state key before exposing the draft.
    // No transaction has been sent, so a crash here cannot spend or double-publish.
    save_storage(&root, &storage)?;
    let policy = Policy {
        kind,
        state_account: state_hex.clone(),
        root: hex::encode(merkle_root),
        title: args.title,
        field_name: args.field_name,
        value_type: kind.value_type().into(),
        initial_value: args.initial_value,
        member_count: leaves.len() as u32,
        threshold: args.threshold,
        creator: true,
        invitations,
    };
    write_new(
        &root.join(policy_file(&state_hex)?),
        &serde_json::to_vec_pretty(&policy)?,
    )?;
    Ok(state_hex)
}
fn join_policy(home: &Path, profile: &str, invitation: Invitation) -> Result<()> {
    let root = checked_profile(home, profile)?;
    let _lock = WalletLock::take(&root)?;
    ensure!(
        invitation.schema_version == 1
            && invitation.program_id == program_id(invitation.kind)?
            && invitation.value_type == invitation.kind.value_type(),
        "Unsupported invitation or program."
    );
    label(&invitation.title)?;
    label(&invitation.field_name)?;
    integer(&invitation.initial_value)?;
    if invitation.kind == WorkspaceKind::Allowlist {
        ensure!(
            invitation.initial_value == "0"
                && invitation.threshold == 1
                && invitation.field_name == "membership",
            "Invalid membership-list invitation."
        );
    }
    let n = invitation.member_count;
    ensure!(
        n > 0
            && n <= 256
            && invitation.threshold > 0
            && invitation.threshold <= n
            && invitation.leaf_index < n,
        "Invalid membership policy."
    );
    let leaf = invitation.member.leaf()?;
    let scope = context(invitation.program_id, account(&invitation.state_account)?);
    let depth = n.next_power_of_two().trailing_zeros() as usize;
    ensure!(
        invitation.siblings.len() == depth,
        "Wrong membership proof depth."
    );
    let siblings: Vec<[u8; 32]> = invitation
        .siblings
        .iter()
        .map(|s| hash(s))
        .collect::<Result<_>>()?;
    let mut current = leaf.commitment(&scope);
    let mut index = invitation.leaf_index;
    for other in &siblings {
        current = if index & 1 == 0 {
            hash_parts(b"commons/node/v1", &[&current, other])
        } else {
            hash_parts(b"commons/node/v1", &[other, &current])
        };
        index >>= 1;
    }
    ensure!(
        current == hash(&invitation.root)?,
        "Invitation does not match its membership commitment."
    );
    let storage = Storage::from_path(&root.join("storage.json"))?;
    let found = storage
        .key_chain()
        .private_account(leaf.account_id)
        .context("This invitation is for a different identity. Select the intended member.")?;
    let identifier = match found.kind {
        PrivateAccountKind::Regular(id) => *id,
        _ => anyhow::bail!("This onboarding flow accepts regular private accounts only."),
    };
    let witness = MemberWitness {
        leaf,
        nullifier_secret_key: found.key_chain.private_key_holder.nullifier_secret_key,
        viewing_public_key: ViewingPublicKeyBytes::from_viewing_public_key(
            &found.key_chain.viewing_public_key,
        ),
        identifier,
        leaf_index: invitation.leaf_index,
        siblings,
    };
    // Reject conflicting local metadata before creating any credential file.
    let existing_policy = root.join(policy_file(&invitation.state_account)?);
    if existing_policy.exists() {
        let old: Policy = load(&root, &policy_file(&invitation.state_account)?)?;
        ensure!(
            old.kind == invitation.kind
                && old.root == invitation.root
                && old.member_count == n
                && old.threshold == invitation.threshold,
            "Existing policy does not match this invitation."
        );
    } else {
        ensure!(policies(&root)?.len() < 256, "Policy limit reached.");
    }
    let file = root.join(witness_file(&invitation.state_account)?);
    let bytes = borsh::to_vec(&witness)?;
    if file.exists() {
        ensure!(
            read_inside(&root, &file, 16 * 1024, true)? == bytes,
            "A different credential already exists. Nothing was replaced."
        );
    } else {
        write_new(&file, &bytes)?;
    }
    let path = root.join(policy_file(&invitation.state_account)?);
    if path.exists() {
        let old: Policy = load(&root, &policy_file(&invitation.state_account)?)?;
        ensure!(
            old.kind == invitation.kind
                && old.root == invitation.root
                && old.member_count == n
                && old.threshold == invitation.threshold,
            "Existing policy does not match this invitation."
        );
    } else {
        ensure!(policies(&root)?.len() < 256, "Policy limit reached.");
        let policy = Policy {
            kind: invitation.kind,
            state_account: invitation.state_account.clone(),
            root: invitation.root.clone(),
            title: invitation.title.clone(),
            field_name: invitation.field_name.clone(),
            value_type: invitation.kind.value_type().into(),
            initial_value: invitation.initial_value.clone(),
            member_count: n,
            threshold: invitation.threshold,
            creator: false,
            invitations: vec![invitation],
        };
        atomic_json(&path, &policy)?;
    }
    Ok(())
}

pub async fn dispatch(action: &str, bytes: &[u8]) -> Result<Value> {
    ensure!(
        bytes.len() as u64 <= LIMIT,
        "Governance request is too large."
    );
    let request: Request = serde_json::from_slice(bytes)?;
    ensure!(
        request.schema_version == 1,
        "Unsupported governance schema."
    );
    let home = checked_home(&request.home)?;
    // A process-wide home lease prevents concurrent profile/catalog writes.
    let _home_lock = WalletLock::take(&home)?;
    let mut selected_workspace: Option<String> = None;
    let selected = match action {
        "catalog" => {
            ensure!(
                request.arguments == json!({}),
                "Catalog takes no arguments."
            );
            request.profile
        }
        "create-identity" => {
            create_identity(&home, serde_json::from_value(request.arguments)?).await?
        }
        "prepare-policy" => {
            selected_workspace = Some(prepare_policy(
                &home,
                &request.profile,
                serde_json::from_value(request.arguments)?,
                WorkspaceKind::Threshold,
            )?);
            request.profile
        }
        "prepare-list" => {
            let args: PrepareList = serde_json::from_value(request.arguments)?;
            selected_workspace = Some(prepare_policy(
                &home,
                &request.profile,
                PreparePolicy {
                    title: args.title,
                    field_name: "membership".into(),
                    initial_value: "0".into(),
                    threshold: 1,
                    members: args.members,
                },
                WorkspaceKind::Allowlist,
            )?);
            request.profile
        }
        "join-policy" => {
            let args: JoinPolicy = serde_json::from_value(request.arguments)?;
            selected_workspace = Some(hex::encode(hash(&args.invitation.state_account)?));
            join_policy(&home, &request.profile, args.invitation)?;
            request.profile
        }
        _ => anyhow::bail!("Unknown governance setup action."),
    };
    let mut result = catalog(&home, &selected)?;
    if let Some(state) = selected_workspace {
        result["selected_workspace"] = json!(state);
    }
    result["message"] = json!(match action {
        "create-identity" =>
            "Identity created locally. Share only its enrollment, never the wallet folder.",
        "prepare-policy" => "Policy draft saved. Review it before publishing on testnet.",
        "prepare-list" => "Membership-list draft saved. Review it before publishing on testnet.",
        "join-policy" =>
            "Invitation verified and private credential saved. No vote or transaction was sent.",
        _ => "Governance library loaded.",
    });
    Ok(result)
}

#[derive(Debug)]
pub struct PublicIssue(pub &'static str);
impl std::fmt::Display for PublicIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for PublicIssue {}
pub fn public_issue(error: &anyhow::Error) -> PublicIssue {
    // Only exact, locally authored messages can cross the UI boundary. An
    // upstream error is never echoed: it might include a path or private data.
    let text = error.to_string();
    let message = match text.as_str() {
        "The same member cannot occupy two seats." => "The same member cannot occupy two seats.",
        "Choose between 1 and 256 members and a threshold between 1 and that count." => {
            "Choose between 1 and 256 members and a threshold between 1 and that count."
        }
        "This invitation is for a different identity. Select the intended member." => {
            "This invitation is for a different identity. Select the intended member."
        }
        "Invitation does not match its membership commitment." => {
            "Invitation does not match its membership commitment. Ask the organizer for the original invitation."
        }
        "Install the matching program artifacts next to commons-logos-cli before creating an identity." => {
            "The matching program artifacts are missing. Install the complete Commons client package, including its artifacts folder."
        }
        "Use a short label without control characters." => {
            "Use a short label without control characters."
        }
        "An identity with that name already exists. Choose it from the identity list or use a different name." => {
            "An identity with that name already exists. Choose it from the identity list or use a different name."
        }
        "WALLET_BUSY" => {
            "This identity is busy in another window. Let its current operation finish."
        }
        "Governance folder must be private to its owner (mode 700)." => {
            "The governance folder must be private to its owner (mode 700)."
        }
        "Testnet protocol changed; no new identity created." => {
            "The testnet protocol changed. No new identity was created; check for a compatible release."
        }
        "A different credential already exists. Nothing was replaced." => {
            "A different credential already exists. Nothing was replaced."
        }
        "Existing policy does not match this invitation." => {
            "The invitation conflicts with the policy already saved here. Nothing was replaced."
        }
        "canonical integer string required"
        | "number too large to fit in target type"
        | "number too small to fit in target type" => {
            "Use a whole number between -9223372036854775808 and 9223372036854775807, without leading zeros."
        }
        _ => {
            "Governance setup did not finish. Check the selected identity, enrollment or invitation format, matching release artifacts, and testnet connection. No transaction was submitted."
        }
    };
    PublicIssue(message)
}
