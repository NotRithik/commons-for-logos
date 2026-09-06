fn main() {
    // The official builder uses the already-installed local RISC0 toolchain.
    // No install script, shell download, or Docker configuration is supplied.
    risc0_build::embed_methods();
}
