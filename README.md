# Astra Logos Primitives

**Work in progress. Not yet a qualified Lambda Prize submission. Do not use with real funds.**

Two original reference applications for Logos Execution Zone:

- **Private Allowlist**: prove membership of a salted Merkle commitment and register once without publishing the member identity.
- **Private Threshold**: create a proposal, gather anonymous distinct approvals, and execute a threshold-authorized integer parameter update once.

Neither program moves tokens. The first implements the allowlist-gate option; the second implements the parameter-change option in the published prize specifications.

## Current verified status

42 host-side state-machine tests pass on macOS arm64 with the pinned LEZ core. These tests simulate applying program outputs and **are not sequencer, zero-knowledge-proof, or testnet evidence**. Guest, standalone, testnet, Basecamp, cross-platform packaging, and narrated-demo gates are tracked separately.

## Protocol pin

LEZ: `6cec69040cd0af5fb75b2a609f15620b8a1c582f` (dev, 2026-09-06 snapshot). RISC0 SDK: `3.0.5`. Rust: `1.94.0`. Exact transitive versions are in `Cargo.lock`.

## Tests

```sh
cargo test --locked -p astra-logos-primitives --features host
cargo fmt --all -- --check
```

The suite covers membership, private-account binding, duplicate prevention, malformed proofs, state ownership, unchanged member-account data, proposal lifecycle, partial-approval persistence, single-use execution, rejection atomicity, and all supported Merkle tree sizes.

## Important privacy boundary

Claim, propose, and approve instructions contain a private witness. **Never submit those instruction bytes in a public LEZ transaction or put them in logs.** The guest checks the nullifier secret against the actual authorized private account, but private transport/proving is still the caller's responsibility. Public registry state contains the commitment root, counters, application nullifiers, and proposal parameters, not member identities. Timing, transaction shape, and invoked program IDs can remain visible. This is not a claim of network-level anonymity.

The fixture seeds in tests are intentionally public, synthetic test data and must never control value. The implementation and tests were written and run by an AI assistant acting for the repository owner; the owner has not been represented as personally auditing the code.

## License

Dual-licensed under MIT and Apache-2.0. See `LICENSE-MIT` and `LICENSE-APACHE`.
