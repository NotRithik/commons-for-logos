use anyhow::{Context, Result, bail, ensure};
use lee_core::{account::AccountId, program::ProgramId};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const MAX_REQUEST: u64 = 64 * 1024;
pub const MAX_WITNESS: u64 = 16 * 1024;
pub const TESTNET_URL: &str = "https://testnet.lez.logos.co/";
pub const LOCAL_URL: &str = "http://127.0.0.1:34341/";
pub const PROTOCOL_ID: ProgramId = [
    1334328888, 3910590567, 1244219104, 3671232111, 3138827701, 405554639, 4064616947, 1864368340,
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub network: String,
    pub wallet_dir: PathBuf,
    pub operation: String,
    pub arguments: BTreeMap<String, Value>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    CreateDistribution,
    Claim,
    InspectDistribution,
    CreateGroup,
    Propose,
    Approve,
    Execute,
    InspectGroup,
}
impl Op {
    pub fn from_cli(name: &str) -> Result<Self> {
        Ok(match name {
            "allowlist-create" => Self::CreateDistribution,
            "allowlist-claim" => Self::Claim,
            "allowlist-inspect" => Self::InspectDistribution,
            "threshold-create" => Self::CreateGroup,
            "threshold-propose" => Self::Propose,
            "threshold-approve" => Self::Approve,
            "threshold-execute" => Self::Execute,
            "threshold-inspect" => Self::InspectGroup,
            _ => bail!("unknown operation"),
        })
    }
    pub const fn id(self) -> &'static str {
        match self {
            Self::CreateDistribution => "allowlist.create_distribution",
            Self::Claim => "allowlist.claim",
            Self::InspectDistribution => "allowlist.inspect_state",
            Self::CreateGroup => "threshold.create_group",
            Self::Propose => "threshold.propose",
            Self::Approve => "threshold.approve",
            Self::Execute => "threshold.execute",
            Self::InspectGroup => "threshold.inspect_state",
        }
    }
    pub const fn family(self) -> &'static str {
        match self {
            Self::CreateDistribution | Self::Claim | Self::InspectDistribution => "allowlist",
            _ => "threshold",
        }
    }
    pub const fn is_private(self) -> bool {
        matches!(self, Self::Claim | Self::Propose | Self::Approve)
    }
    pub const fn is_read(self) -> bool {
        matches!(self, Self::InspectDistribution | Self::InspectGroup)
    }
    pub const fn keys(self) -> &'static [&'static str] {
        match self {
            Self::CreateDistribution => &["state_account", "root", "member_count"],
            Self::CreateGroup => &[
                "state_account",
                "root",
                "member_count",
                "threshold",
                "initial_value",
            ],
            Self::Claim | Self::Approve => &["state_account", "witness_file"],
            Self::Propose => &["state_account", "witness_file", "next_value"],
            _ => &["state_account"],
        }
    }
}
pub fn string<'a>(args: &'a BTreeMap<String, Value>, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .context("string argument required")
}
pub fn hash(value: &str) -> Result<[u8; 32]> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    ensure!(value.len() == 64, "exactly 32-byte hex required");
    Ok(hex::decode(value)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid hash"))?)
}
pub fn account(value: &str) -> Result<AccountId> {
    Ok(AccountId::new(hash(value)?))
}
pub fn count(args: &BTreeMap<String, Value>, key: &str) -> Result<u32> {
    let n = args
        .get(key)
        .and_then(Value::as_u64)
        .context("unsigned integer required")?;
    ensure!((1..=256).contains(&n), "count out of range");
    Ok(n as u32)
}
pub fn integer(value: &str) -> Result<i64> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    ensure!(
        !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && (digits == "0" || !digits.starts_with('0')),
        "canonical integer string required"
    );
    value.parse().map_err(Into::into)
}
pub fn validate(req: &Request, op: Op) -> Result<()> {
    ensure!(
        req.schema_version == 1 && req.network == "testnet",
        "testnet schema1 required"
    );
    ensure!(req.operation == op.id(), "operation mismatch");
    ensure!(
        req.arguments.len() == op.keys().len()
            && op.keys().iter().all(|k| req.arguments.contains_key(*k)),
        "exact argument set required"
    );
    account(string(&req.arguments, "state_account")?)?;
    if matches!(op, Op::CreateDistribution | Op::CreateGroup) {
        hash(string(&req.arguments, "root")?)?;
        count(&req.arguments, "member_count")?;
    }
    if matches!(op, Op::CreateGroup) {
        ensure!(
            count(&req.arguments, "threshold")? <= count(&req.arguments, "member_count")?,
            "threshold exceeds members"
        );
        integer(string(&req.arguments, "initial_value")?)?;
    }
    if op == Op::Propose {
        integer(string(&req.arguments, "next_value")?)?;
    }
    if op.is_private() {
        ensure!(
            !string(&req.arguments, "witness_file")?.is_empty(),
            "witness path required"
        );
    }
    ensure!(
        req.wallet_dir.is_absolute(),
        "absolute wallet path required"
    );
    Ok(())
}
pub fn wallet_root(path: &Path) -> Result<PathBuf> {
    ensure!(
        path.is_absolute()
            && path.is_dir()
            && !fs::symlink_metadata(path)?.file_type().is_symlink(),
        "regular absolute wallet directory required"
    );
    let root = path.canonicalize()?;
    ensure!(
        root.join(".astra-logos-testnet-wallet").is_file(),
        "testnet wallet marker missing"
    );
    Ok(root)
}
pub fn read_inside(root: &Path, path: &Path, maximum: u64, private: bool) -> Result<Vec<u8>> {
    ensure!(path.is_absolute(), "absolute input path required");
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "regular input file required"
    );
    let canonical = path.canonicalize()?;
    ensure!(
        canonical.starts_with(root) && canonical != root,
        "input outside wallet"
    );
    ensure!(metadata.len() <= maximum, "input exceeds bound");
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "private witness must be owner-only"
        );
    }
    use std::io::Read;
    let mut data = Vec::new();
    fs::File::open(&canonical)?
        .take(maximum + 1)
        .read_to_end(&mut data)?;
    ensure!(data.len() as u64 <= maximum, "input exceeds bound");
    Ok(data)
}
pub fn endpoint_allowed(value: &str, local_enabled: bool) -> bool {
    value == TESTNET_URL || (local_enabled && value == LOCAL_URL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn request(op: Op) -> Request {
        let mut a = BTreeMap::new();
        a.insert("state_account".into(), json!("aa".repeat(32)));
        for k in op.keys() {
            if *k == "state_account" {
                continue;
            }
            a.insert(
                k.to_string(),
                match *k {
                    "root" => json!("bb".repeat(32)),
                    "member_count" => json!(10),
                    "threshold" => json!(2),
                    "initial_value" | "next_value" => json!("42"),
                    _ => json!("/tmp/testnet/witness.bin"),
                },
            );
        }
        Request {
            schema_version: 1,
            network: "testnet".into(),
            wallet_dir: PathBuf::from("/tmp/testnet"),
            operation: op.id().into(),
            arguments: a,
        }
    }
    #[test]
    fn all_operations_match_exact_schema() {
        for op in [
            Op::CreateDistribution,
            Op::Claim,
            Op::InspectDistribution,
            Op::CreateGroup,
            Op::Propose,
            Op::Approve,
            Op::Execute,
            Op::InspectGroup,
        ] {
            validate(&request(op), op).unwrap();
        }
    }
    #[test]
    fn unknown_operation_rejected() {
        assert!(Op::from_cli("send-funds").is_err());
    }
    #[test]
    fn wrong_operation_cannot_select_private_dispatch() {
        assert!(validate(&request(Op::Claim), Op::Execute).is_err());
    }
    #[test]
    fn unknown_argument_rejected() {
        let mut r = request(Op::Claim);
        r.arguments.insert("shell".into(), json!("echo nope"));
        assert!(validate(&r, Op::Claim).is_err());
    }
    #[test]
    fn create_requires_preselected_state_account() {
        let mut r = request(Op::CreateDistribution);
        r.arguments.remove("state_account");
        assert!(validate(&r, Op::CreateDistribution).is_err());
    }
    #[test]
    fn no_mainnet() {
        let mut r = request(Op::Claim);
        r.network = "mainnet".into();
        assert!(validate(&r, Op::Claim).is_err());
    }
    #[test]
    fn schema_version_checked() {
        let mut r = request(Op::Claim);
        r.schema_version = 2;
        assert!(validate(&r, Op::Claim).is_err());
    }
    #[test]
    fn no_relative_wallet() {
        let mut r = request(Op::Claim);
        r.wallet_dir = "relative".into();
        assert!(validate(&r, Op::Claim).is_err());
    }
    #[test]
    fn count_rejects_fractions_strings_zero_negative_and_huge() {
        for n in [
            json!(1.5),
            json!("5"),
            json!(0),
            json!(-1),
            json!(257),
            json!(u64::MAX),
        ] {
            let mut r = request(Op::CreateDistribution);
            r.arguments.insert("member_count".into(), n);
            assert!(validate(&r, Op::CreateDistribution).is_err());
        }
    }
    #[test]
    fn threshold_bounded_by_members() {
        let mut r = request(Op::CreateGroup);
        r.arguments.insert("threshold".into(), json!(11));
        assert!(validate(&r, Op::CreateGroup).is_err());
    }
    #[test]
    fn signed_minimum_and_maximum_are_exact_strings() {
        assert_eq!(integer("-9223372036854775808").unwrap(), i64::MIN);
        assert_eq!(integer("9223372036854775807").unwrap(), i64::MAX);
    }
    #[test]
    fn invalid_integer_spellings_rejected() {
        for v in [
            "+1",
            "01",
            "-01",
            "1.0",
            "1e5",
            "",
            "-",
            " 5",
            "9223372036854775808",
            "-9223372036854775809",
        ] {
            assert!(integer(v).is_err(), "{v}");
        }
    }
    #[test]
    fn hashes_require32bytes() {
        assert_eq!(hash(&"ff".repeat(32)).unwrap(), [255; 32]);
        assert_eq!(hash(&format!("0x{}", "00".repeat(32))).unwrap(), [0; 32]);
        for n in [0, 31, 33, 64] {
            assert!(hash(&"aa".repeat(n)).is_err());
        }
    }
    #[test]
    fn invalid_hex_rejected() {
        assert!(hash(&"gg".repeat(32)).is_err());
    }
    #[test]
    fn endpoint_pin_is_exact() {
        for v in [
            "https://testnet.lez.logos.co.evil/",
            "http://testnet.lez.logos.co/",
            "https://testnet.lez.logos.co:444/",
            "https://example.com/",
            "file:///etc/passwd",
        ] {
            assert!(!endpoint_allowed(v, true));
        }
        assert!(endpoint_allowed(TESTNET_URL, false));
        assert!(!endpoint_allowed(LOCAL_URL, false));
        assert!(endpoint_allowed(LOCAL_URL, true));
    }
    #[test]
    fn additional_top_level_fields_rejected() {
        let r = json!({"schema_version":1,"network":"testnet","wallet_dir":"/tmp/testnet","operation":"allowlist.claim","arguments":{},"unknown":true});
        assert!(serde_json::from_value::<Request>(r).is_err());
    }
}

/// Receipt IDs are exactly one 32-byte lowercase or uppercase hex digest.
pub fn validate_pending_tx_id(value: &str) -> Result<()> {
    ensure!(
        value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid pending transaction identifier"
    );
    Ok(())
}

#[cfg(test)]
mod pending_validation_tests {
    use super::*;
    #[test]
    fn valid_receipt_id_has_exact_length_and_hex_encoding() {
        assert!(validate_pending_tx_id(&"ab".repeat(32)).is_ok());
        assert!(validate_pending_tx_id(&"AB".repeat(32)).is_ok());
    }
    #[test]
    fn malformed_or_unbounded_receipt_identifiers_are_refused() {
        for value in [
            "",
            "../../../storage.json",
            "https://example.test",
            &"g".repeat(64),
            &"a".repeat(63),
            &"a".repeat(65),
        ] {
            assert!(validate_pending_tx_id(value).is_err());
        }
    }
}
