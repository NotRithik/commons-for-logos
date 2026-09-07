# Execution cost measurements

The measured unit is **RISC0 user cycles**, as defined by RISC0 session metadata. The runner executes the actual packed guest binaries on deterministic benchmark inputs and validates the returned state transitions. It does not generate proofs or submit network transactions.

RISC0 3.0.5 defines this counter as guest user cycles excluding continuation overhead and power-of-two padding. The outer LEZ privacy circuit, proof compression and network confirmation are additional work; they are not included in this table.

| Operation | 3 members | 10 members | 256 members |
|---|---:|---:|---:|
| allowlist.create | 59,923 | 59,923 | 59,923 |
| allowlist.claim | 554,527 | 592,883 | 669,595 |
| threshold.create | 63,340 | 63,340 | 63,340 |
| threshold.propose | 546,826 | 585,182 | 661,894 |
| threshold.approve_first | 570,893 | 609,249 | 685,961 |
| threshold.approve_second | 584,572 | 622,928 | 699,640 |
| threshold.execute | 116,216 | 116,216 | 116,216 |

The membership cases measure the first registration and a two-approval threshold. Cost can also increase with accumulated nullifiers and larger account state. Creation and execution do not traverse the membership tree in these fixtures.

## Reproduce

```sh
cargo +1.94.0 run --locked --manifest-path integration/Cargo.toml \
  --bin measure_guests -- out/artifacts > guest-cycles.json
```

Use the same packed program images as the deployment ledger. The raw output includes each image ID, fixture size and executor wall time. It never exports inner journals or witness bytes.

## Real transaction timing

Confirmed integration events record proof-plus-submission wall time and accepted block IDs. Those measurements depend on the prover hardware and network load. They are recorded separately from these deterministic execution-cycle counts.

Counter definition: `risc0-zkvm` 3.0.5, `src/host/api/mod.rs`, `SessionInfo::cycles`. Source: https://github.com/risc0/risc0/blob/v3.0.5/risc0/zkvm/src/host/api/mod.rs
