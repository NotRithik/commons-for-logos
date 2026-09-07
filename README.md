# Astra Logos Primitives

**Testnet reference implementation. Work in progress; not an accepted Lambda Prize solution and not audited. Never use these fixture wallets with real funds.**

Two original Logos Execution Zone applications, with a real native Basecamp GUI:

- **Private allowlist gate:** commit a salted eligibility tree, prove membership locally, and register each eligible identity once without publishing the member address.
- **Private threshold control:** propose an integer parameter update, collect distinct private approvals, and execute the update once the configured threshold is reached.

Neither program transfers user tokens. These implement the allowlist-gate and parameter-change examples in LP-0003 and LP-0002. Account creation/deployment on the configured network may use testnet resources.

## Use the correct protocol version

**`testnet/` is the canonical implementation for the public testnet.** It pins LEZ v0.2.4 at `47eba256479f6f785acbd138834340703cd03401`, RISC0 3.0.5 and Rust 1.94.0. The official endpoint is `https://testnet.lez.logos.co/`.

The root `crates/` workspace preserves an earlier experiment against a different LEZ development ABI. Do not deploy its binaries to the public testnet or combine its Borsh guest-call encoding with the v0.2.4 SDK. Each workspace has its own lockfile.

## Current evidence, not promises

The corrected v2 programs have been deployed on the official testnet. Their program IDs and deployment receipts are in [the verification ledger](evidence/verification.json). A real native Basecamp group-creation action was independently read back from the public sequencer. A corrected private proposal has also been proven and confirmed against the real local sequencer with `RISC0_DEV_MODE=0`.

The complete 20-claim/two-distribution public flow, threshold execution, final narrated demonstrations and full real-proof CI are still being verified. **A running proof job, passing host test, green build or repository publication is not counted as a completed prize criterion.**

Latest locally verified suites: 55 canonical primitive tests, 2 SDK serialization tests, 18 production CLI validation/recovery tests, 27 native Qt process-boundary checks, and 3 SPEL IDL tests. The root experimental workspace has a separate 42-test suite. See the repository's Actions results for the exact revision and platforms actually checked; do not infer cross-platform or end-to-end success from these counts.

## Project layout

| Directory | Purpose |
| --- | --- |
| `testnet/crates/primitives` | Shared v0.2.4 instruction types, state transition rules and regression tests |
| `testnet/crates/guest` | Actual RISC-V LEZ guest programs |
| `testnet/crates/sdk` | Typed Rust instruction construction, wire encoding and guest packaging |
| `cli` | Production JSON-over-stdin CLI, endpoint checks, wallet locking and broadcast recovery |
| `integration` | Real sequencer/testnet demo runner and independent state verification |
| `sdk/src` | Native Qt SDK used by the GUI; fixed allowlisted subprocess interface, no shell |
| `module` | Basecamp Qt Remote Objects backend and QML view |
| `native`, `scripts` | Reproducible native plugin build with pinned official dependencies |
| `idl` | SPEL-generated public testnet instruction and error schemas |
| `docs` | Protocol, privacy model, error semantics, GUI and build instructions |

## Test the canonical implementation

```sh
cargo +1.94.0 test --locked --manifest-path testnet/Cargo.toml \
  -p astra-logos-testnet-primitives -p astra-logos-testnet-sdk
cargo +1.94.0 test --locked --manifest-path cli/Cargo.toml
cargo +1.94.0 test --locked --manifest-path idl/Cargo.toml
cargo +1.94.0 run --locked --manifest-path idl/Cargo.toml
git diff --exit-code -- idl/generated
```

The CLI's dependencies include the real LEZ wallet and proving stack; install/fetch their pinned build prerequisites before compiling it. For an offline build, fetch dependencies in a separate step, then pass `--offline`. No hosted prover is required or supported by the CLI.

## Build the native Basecamp module

See [native build instructions](docs/NATIVE-BUILD.md). The tested build produces the actual plugin and replica factory, runs the Qt process-boundary tests, and installs only the module's own files. It does not install the fake test CLI, replace a system Basecamp application, or package the Qt SDK/fonts for distribution.

## Important safety and privacy boundaries

Read [the privacy model](docs/PRIVACY.md) and [protocol specification](docs/PROTOCOL.md) before using the code. In particular:

- Claim/propose/approve instructions contain secrets and go only through local private proving. Never publish a witness, an unwrapped inner journal, a test-wallet storage file, or raw upstream debug output.
- Public state exposes the membership commitment, counts, application nullifiers and proposal parameter, not a list of member addresses. Timing, traffic patterns and outside information are not hidden by this application.
- The pinned upstream wallet stores key data in **plaintext JSON**. This project uses restricted file permissions and separate member-only profiles; that is not encryption at rest. Keep the operating-system account and storage protected.
- The production CLI pins the permitted endpoint and the v0.2.4 protocol fingerprint. It forces real local IPC proving, never invokes a model API, and rejects silently switching to a paid hosted prover.
- This is reference code, not an audit or guarantee of production security. The demonstration setup generates controlled test identities; it does not claim independent human users or production key ceremonies.

Implementation and validation work are AI-assisted and disclosed. The repository owner is not represented as having personally audited the code. Prize evaluation and awards belong to Logos, and no endorsement or payout is implied.

## License

Dual-licensed under MIT and Apache-2.0; see `LICENSE-MIT` and `LICENSE-APACHE`. Upstream dependencies keep their own licenses.
