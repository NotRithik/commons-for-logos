# Commons for Logos

Private membership and shared approvals in Logos Basecamp.

Commons has two applications. **Allowlist** lets a member register once by proving membership of a committed eligibility set. **Shared approvals** lets a group approve a parameter change with an M-of-N threshold. Both use LEZ private accounts and locally generated RISC Zero proofs. Member addresses and credentials are not written into public application state.

This release targets the Logos testnet. It is not audited and must not hold real funds.

## Start here

The native Basecamp module has private membership, shared approvals, and **My workspaces**. Create or load your own member identity, select a named list or policy, and review the action before submitting. Organizers can prepare lists and 2-of-3-style approval policies from member enrollments without collecting anyone else's signing key. Each participant imports their own invitation locally. Raw blockchain references, connection paths and transaction JSON stay in advanced controls.

Start with [the plain-English quick start](docs/QUICK-START.md). It explains identities, drafts, publication, invitations and the difference between registering, proposing, approving and applying a change. The examples are a membership gate and a threshold-controlled integer setting, not token payouts or storage provisioning.

LP-0002 and LP-0003 share this module and SDK but use **different on-chain programs**, separate evidence and separate submissions. Kite / Commons Relay (LP-0008) is a different project and is not included in this module's package.

- [Build and install the native module](docs/NATIVE-BUILD.md)
- [Use the Basecamp app and CLI](docs/GUI.md)
- [Run the complete local demonstration](docs/LOCAL-DEMO.md)
- [Protocol and account model](docs/PROTOCOL.md)
- [Consume an approved decision from another on-chain program](docs/COMPOSABILITY.md)
- [Privacy model](docs/PRIVACY.md)
- [LP-0002 recording and narration plan](docs/RECORDING-LP-0002.md)
- [LP-0003 recording and narration plan](docs/RECORDING-LP-0003.md)

The current testnet addresses are in [release/deployment.json](release/deployment.json).
Run `python3 scripts/check-public-state.py` to inspect them without loading a wallet
or sending a transaction. Its `chain_criteria` fields report the observed shared-approval and membership
acceptance state. Read the returned counts and timestamp rather than inferring
completion from this README. A saved snapshot
is timestamped evidence, not a promise about later testnet resets.

The 7 September twenty-claim and shared-approval receipts remain **historical** in
`evidence/testnet-completed.json` and `evidence/verification.json`; their pre-reset
addresses are preserved in `release/deployment-pre-reset-20260907.json`. Do not use
those historical counts as evidence that the current network has twenty claims.

## Reproduce the local stack

From a clean clone, with the prerequisites in the local demonstration guide:

```sh
/bin/sh scripts/prepare-local.sh fetch
/bin/sh scripts/prepare-local.sh build
./demo.sh --mode all
```

The first command downloads pinned dependencies. The second compiles offline. The third starts its own local sequencer, deploys both programs, completes private claims and a threshold decision, then stops that sequencer. It uses fresh test profiles and real local proofs (`RISC0_DEV_MODE=0`).

## Prize entrypoints

For LP-0002, run `./demo.sh --mode threshold`. For LP-0003, run
`./demo.sh --mode allowlist-smoke`. The latter exercises one real private claim
in each of two local lists; the two-list, twenty-claim public-testnet evidence is
separate. Run `./demo.sh --help` to inspect the options without starting a proof.
The prerequisite fetch and build commands above are required for either mode.

The native source manifest is [module/module.json](module/module.json), identical
to the `metadata.json` used by the CMake build. The `*.idl.json` files in
[idl/generated](idl/generated) are byte-identical named copies of the existing
SPEL-generated interfaces. Regenerate their matching `.json` sources through
`idl/` when changing the interfaces, then refresh the copies.

- [LP-0002 submission and evidence](docs/SUBMISSION-LP-0002.md)
- [LP-0003 submission and evidence](docs/SUBMISSION-LP-0003.md)
- [Short recording outlines for both prizes](docs/RECORDING-MINIMUM.md)

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

Dual-licensed under [MIT](LICENSE-MIT) and [Apache-2.0](LICENSE-APACHE), at your option. Upstream dependencies retain their own licenses.
