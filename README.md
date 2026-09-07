# Commons for Logos

Private membership and shared approvals in Logos Basecamp.

Commons has two applications. **Allowlist** lets a member register once by proving membership of a committed eligibility set. **Shared approvals** lets a group approve a parameter change with an M-of-N threshold. Both use LEZ private accounts and locally generated RISC Zero proofs. Member addresses and credentials are not written into public application state.

This release targets the Logos testnet. It is not audited and must not hold real funds.

## Start here

The native Basecamp module has an allowlist screen, a shared-approvals screen, and a local client connection. Select your wallet profile, inspect a distribution or group, and choose the membership credential stored on your device. Connection paths and transaction JSON stay in collapsible settings and technical details.

- [Build and install the native module](docs/NATIVE-BUILD.md)
- [Use the Basecamp app and CLI](docs/GUI.md)
- [Run the complete local demonstration](docs/LOCAL-DEMO.md)
- [Protocol and account model](docs/PROTOCOL.md)
- [Privacy model](docs/PRIVACY.md)

Deployment addresses, confirmed transactions and completed verification runs are recorded in [the release evidence](evidence/verification.json). Only results for the program images named there apply to this release.

## Reproduce the local stack

From a clean clone, with the prerequisites in the local demonstration guide:

```sh
/bin/sh scripts/prepare-local.sh fetch
/bin/sh scripts/prepare-local.sh build
python3 scripts/demo-local.py --mode all
```

The first command downloads pinned dependencies. The second compiles offline. The third starts its own local sequencer, deploys both programs, completes private claims and a threshold decision, then stops that sequencer. It uses fresh test profiles and real local proofs (`RISC0_DEV_MODE=0`).

## Tests

```sh
cargo +1.94.0 test --locked --manifest-path testnet/Cargo.toml \
  -p commons-logos-testnet-primitives -p commons-logos-testnet-sdk
cargo +1.94.0 test --locked --manifest-path cli/Cargo.toml
cargo +1.94.0 test --locked --manifest-path idl/Cargo.toml
cargo +1.94.0 run --locked --manifest-path idl/Cargo.toml
python3 -m unittest discover -s tests/tooling -v
```

The native build also runs the Qt client-boundary and UI-state regression suites. Those tests use a labeled fixture executable; the installed module uses the real `commons-logos-cli`.

## Layout

`testnet/` contains the LEZ programs and Rust SDK. `cli/` provides validated JSON-over-stdin operations. `integration/` drives reproducible real-chain verification. `sdk/`, `module/` and `native/` contain the native Qt client, Basecamp view and build definitions. `idl/` generates the SPEL interfaces.

The protocol is pinned to LEZ v0.2.4, revision `47eba256479f6f785acbd138834340703cd03401`, with RISC0 3.0.5 and Rust 1.94.0. Do not replace this dependency pin without checking the wire format and private-account behavior.

## Before using a profile

Credentials and wallet files stay local. The pinned LEZ wallet stores keys in plaintext JSON; restricted file permissions are not encryption. Keep those files out of source control and recordings. Claim and approval payloads must use the private proving path. Public observers can still see group parameters, counters and transaction timing; see the privacy guide for the precise assumptions.

Implementation and testing were AI-assisted. Commons is independently developed for the Logos ecosystem.

## License

MIT or Apache-2.0, at your option. Upstream dependencies retain their own licenses.
