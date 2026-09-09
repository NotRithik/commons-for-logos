//! Export existing witnesses into private, GUI-selectable files.
//! No wallet loading, identity generation, network requests, or secret output.
use borsh::BorshDeserialize;
use commons_logos_testnet_primitives::MemberWitness;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn protected_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() || meta.file_type().is_symlink() || meta.permissions().mode() & 0o077 != 0 {
        return Err("Output directory must be private and not a symlink".into());
    }
    Ok(())
}
fn export(root: &Path, family: &str) -> Result<usize> {
    if !["threshold", "distribution_a", "distribution_b"].contains(&family) {
        return Err("Expected threshold, distribution_a or distribution_b".into());
    }
    if !root.is_absolute() || fs::symlink_metadata(root)?.file_type().is_symlink() {
        return Err("A regular absolute testnet profile is required".into());
    }
    let root = root.canonicalize()?;
    if !root.join(".commons-logos-testnet-wallet").is_file() {
        return Err("The directory is not a marked testnet profile".into());
    }
    let input = root.join(format!("{family}-witnesses.bin"));
    let meta = fs::symlink_metadata(&input)?;
    if !meta.is_file()
        || meta.file_type().is_symlink()
        || meta.len() > 1_000_000
        || meta.permissions().mode() & 0o077 != 0
    {
        return Err("The witness bundle must be a private regular file".into());
    }
    let witnesses = Vec::<MemberWitness>::try_from_slice(&fs::read(&input)?)?;
    if witnesses.is_empty() || witnesses.len() > 256 {
        return Err("Witness count is outside the supported range".into());
    }
    let parent = root.join("ui-credentials");
    protected_directory(&parent)?;
    let dest = parent.join(family);
    protected_directory(&dest)?;
    for (index, witness) in witnesses.iter().enumerate() {
        let bytes = borsh::to_vec(witness)?;
        let file = dest.join(format!("member-{}.borsh", index + 1));
        if file.exists() {
            let meta = fs::symlink_metadata(&file)?;
            if !meta.is_file()
                || meta.file_type().is_symlink()
                || meta.permissions().mode() & 0o077 != 0
                || fs::read(&file)? != bytes
            {
                return Err(
                    "An existing credential differs or has unsafe permissions; it was not replaced"
                        .into(),
                );
            }
        } else {
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&file)?;
            out.write_all(&bytes)?;
            out.sync_all()?;
        }
    }
    Ok(witnesses.len())
}
fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().ok_or("Absolute profile path required")?);
    let family = args.next().ok_or("Witness family required")?;
    if args.next().is_some() {
        return Err("Unexpected argument".into());
    }
    let count = export(&root, &family)?;
    println!(
        "Exported {count} credentials into ui-credentials/{family}/. Wallet/network calls: 0. Keep these files private."
    );
    Ok(())
}
fn main() {
    if run().is_err() {
        eprintln!(
            "Credential export failed. Check the protected profile, bundle and output permissions; existing wallet and bundle were preserved."
        );
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use commons_logos_testnet_primitives::{MemberLeaf, ViewingPublicKeyBytes};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "commons-credential-fixture-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&dir).unwrap();
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
            fs::write(
                dir.join(".commons-logos-testnet-wallet"),
                "test fixture only",
            )
            .unwrap();
            Self(dir)
        }
        fn bundle(&self) {
            let member = MemberWitness {
                leaf: MemberLeaf {
                    account_id: BorshDeserialize::try_from_slice(&[0u8; 32]).unwrap(),
                    salt: [0; 32],
                    entitlement: 1,
                },
                nullifier_secret_key: [0; 32],
                viewing_public_key: ViewingPublicKeyBytes::new(vec![0; ViewingPublicKeyBytes::LEN])
                    .unwrap(),
                identifier: 1,
                leaf_index: 0,
                siblings: vec![],
            };
            let p = self.0.join("threshold-witnesses.bin");
            fs::write(&p, borsh::to_vec(&vec![member]).unwrap()).unwrap();
            fs::set_permissions(p, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn exact_roundtrip_and_idempotent_export() {
        let f = Fixture::new();
        f.bundle();
        let before = fs::read(f.0.join("threshold-witnesses.bin")).unwrap();
        assert_eq!(export(&f.0, "threshold").unwrap(), 1);
        assert_eq!(export(&f.0, "threshold").unwrap(), 1);
        let file = f.0.join("ui-credentials/threshold/member-1.borsh");
        assert!(MemberWitness::try_from_slice(&fs::read(&file).unwrap()).is_ok());
        assert_eq!(
            fs::metadata(file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            before,
            fs::read(f.0.join("threshold-witnesses.bin")).unwrap()
        );
    }
    #[test]
    fn refuses_world_readable_bundle() {
        let f = Fixture::new();
        f.bundle();
        fs::set_permissions(
            f.0.join("threshold-witnesses.bin"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(export(&f.0, "threshold").is_err());
    }
    #[test]
    fn refuses_unmarked_profile_and_arbitrary_family() {
        let f = Fixture::new();
        f.bundle();
        assert!(export(&f.0, "../../elsewhere").is_err());
        fs::remove_file(f.0.join(".commons-logos-testnet-wallet")).unwrap();
        assert!(export(&f.0, "threshold").is_err());
    }
    #[test]
    fn preserves_existing_mismatched_credential() {
        let f = Fixture::new();
        f.bundle();
        export(&f.0, "threshold").unwrap();
        let p = f.0.join("ui-credentials/threshold/member-1.borsh");
        fs::write(&p, b"original").unwrap();
        assert!(export(&f.0, "threshold").is_err());
        assert_eq!(fs::read(p).unwrap(), b"original");
    }
    #[test]
    fn rejects_symlinked_output() {
        let f = Fixture::new();
        f.bundle();
        std::os::unix::fs::symlink(&f.0, f.0.join("ui-credentials")).unwrap();
        assert!(export(&f.0, "threshold").is_err());
    }
}
