use std::{env, error::Error, fs, path::PathBuf};

use risc0_binfmt::ProgramBinary;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let raw_elf = PathBuf::from(args.next().ok_or("expected raw RISC-V ELF path")?);
    let packed_bin = PathBuf::from(args.next().ok_or("expected output .bin path")?);
    if args.next().is_some() {
        return Err("usage: pack-v024 <raw-riscv-elf> <output-program-bin>".into());
    }

    let raw = fs::read(&raw_elf)?;
    let packed = ProgramBinary::new(&raw, risc0_zkos_v1compat::V1COMPAT_ELF).encode();
    let image_id: [u32; 8] = ProgramBinary::decode(&packed)?.compute_image_id()?.into();

    if let Some(parent) = packed_bin.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&packed_bin, &packed)?;

    println!(
        "packed_program={} bytes={} image_id={:?}",
        packed_bin.display(),
        packed.len(),
        image_id
    );

    Ok(())
}
