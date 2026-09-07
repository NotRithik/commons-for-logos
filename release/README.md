# Release program identity

`manifest.json` identifies the packaged LEZ v0.2.4 programs deployed for the
0.1.0 release candidate. The SHA256 hashes cover the complete files accepted by
the sequencer, including the pinned RISC0 v1-compatibility wrapper.

Build them through `scripts/build-rust.py` or `scripts/prepare-local.sh`. The
helper normalizes checkout and Cargo-cache paths before invoking the guest
compiler. Calling Cargo directly from a different path can produce a different
image ID even when the Rust logic is unchanged.

Two separate local checkouts produced byte-identical guest ELF files. The
comparison is recorded in `evidence/build/path-reproducibility.json`. Public
acceptance tests also require the rebuilt packaged files to match this
manifest before submitting any transaction.

The `Public testnet allowlist acceptance` workflow is manual and only runs on a
public repository using standard GitHub-hosted Ubuntu runners. Each of its two
jobs creates its own fresh test identities and ten claims in a distinct
allowlist. It does not import a maintainer's wallet, upload private logs or
credentials, use model APIs, or claim independent human adoption. The output is
a public transaction receipt report, not a payment claim.

A green compilation job alone does not establish completed testnet activity.
Read the final acceptance result and independently check the listed transactions.
