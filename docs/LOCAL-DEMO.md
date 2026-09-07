# Real local demonstration

The integration command starts the pinned LEZ sequencer, deploys the compiled
RISC-V programs, generates RISC0 proofs locally, submits private transactions and
checks their confirmations. It uses fresh local profiles and no public network.
`RISC0_DEV_MODE=0`, `RISC0_PROVER=ipc` and `RISC0_EXECUTOR=ipc` are mandatory.

## Prerequisites

Use Linux x86_64 or Apple Silicon macOS, Python 3.12+, Git, curl, Rustup with Rust
1.94.0, and a C/C++ compiler. On Linux the standard system packages are
`build-essential clang libclang-dev pkg-config libssl-dev libpcsclite-dev libgmp-dev libomp-dev unzip`.
The scripts fetch SHA256- and size-pinned official RISC0, Logos circuits and
rapidsnark archives. They do not run a global installer or replace an existing
Rust installation. `integration/prerequisites.json` records the exact provenance.

From a clean clone:

```sh
/bin/sh scripts/prepare-local.sh fetch
/bin/sh scripts/prepare-local.sh build
python3 scripts/demo-local.py --mode all
```

Fetch and build are separate on purpose: the build phase uses Cargo offline mode,
and can additionally be wrapped in an operating-system sandbox with network
access denied. Cargo offline mode by itself is not an OS security boundary.

The runner requires port 34341 to be unused. It refuses to stop an existing service.
It creates a new owner-only test-wallet directory for every run, starts only its
own sequencer and stops only processes it started. All generated project files
stay under the clone's `out/` directory. Upstream wallet storage is plaintext;
restricted file permissions are not encryption. Never fund these fixture wallets.

`--mode allowlist-smoke` exercises a real private claim in each of two distributions.
`--mode threshold` exercises proposal, two distinct private approvals and execution.
`--mode all` runs both. The default deadline is five hours; use
`--timeout-seconds 7200` for a two-hour run. CPU proving can take tens of minutes
per transaction. A timeout is a failure, not a skipped success.

## Evidence and privacy

The public result is `out/latest-local-report.json`. It reports the observed
success/failure and includes only whitelisted transaction receipts and timings.
Raw upstream output, witness files, synthetic keys and unwrapped inner journals
must never be published. The script retains raw local logs in the owner-only run
directory, not in the public report.

The `Real local sequencer and private proofs` GitHub workflow runs these commands
on a standard public-repository runner, without artifact/cache upload actions.
It prints only the sanitized report. A green host/unit-test workflow is not a
substitute for this real-proof workflow.

The verification ledger identifies completed runs by source revision. Check that
revision before comparing the result with another build.
