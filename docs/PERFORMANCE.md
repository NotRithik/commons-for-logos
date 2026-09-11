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

## Testnet CU reporting

The pinned v0.2.4 sequencer RPC exposes `getTransaction` as a serialized
transaction plus accepted block ID. It does not return a billed-CU counter or a
fee receipt. Its declared methods are in
`lez/sequencer/service/rpc/src/lib.rs` at the pinned revision. The public executor
limits execution to 32 million cycles in `lee/state_machine/src/program/mod.rs`;
that limit is not evidence of how many units a specific transaction consumed.

For this deployment, a per-operation billed-CU value is **not available through
that interface**. The table above documents the reproducible guest-cycle
measurements instead. The inner membership guest, outer privacy circuit, proof
compression and network confirmation are different costs and are not summed
into a made-up CU figure.

## Upstream compute-receipt limitation

The missing per-transaction consumed-CU field is tracked in the open upstream
[LEZ issue 840](https://github.com/logos-blockchain/logos-execution-zone/issues/840),
which identifies the pinned revision and asks for either metered receipts or an
explicit statement of the intended current-testnet CU metric. This repository
reports the measured guest cycles and real proof/confirmation timings separately;
it does not invent a billed-CU conversion. The issue is an acknowledged limitation,
not evidence that the prize's compute-reporting interpretation has been accepted.

## Native-UI timings, 11 September 2026

These are measured elapsed times for the actual native client flow, including
proving, submission and confirmation where applicable. They are not isolated
proof benchmarks or billed compute units. The policy and membership examples
are different public instances.

| Confirmed operation | Block | Elapsed seconds | Private proof path |
|---|---:|---:|---|
| Propose 100 -> 125 | 4325 | 2152.799 | Yes, RISC0_DEV_MODE=0 |
| First independent approval | 4362 | 2101.285 | Yes, RISC0_DEV_MODE=0 |
| Second independent approval | 4426 | 2286.456 | Yes, RISC0_DEV_MODE=0 |
| Execute approved change | 4437 | 35.008 | Public execution, no new member proof |
| Register membership | 4430 | 2233.563 | Yes, RISC0_DEV_MODE=0 |

The separate governed-setting consumer's actual compiled-guest preflights measured
178,942 user cycles for initialization and 186,921 for applying the pinned executed
decision. These execution measurements did not generate private proofs. Its public
transactions confirmed in blocks 4405 and 4406. See the sanitized adapter evidence
and `VERIFICATION.md` for exact scope and public transaction references.
