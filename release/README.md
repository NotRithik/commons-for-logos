# Published testnet programs

`manifest.json` identifies the LEZ v0.2.4 program files deployed for release
`v0.1.0-rc.1`. Its hashes cover the complete files accepted by the sequencer,
including the pinned RISC0 compatibility wrapper. Both macOS and Linux clients
use these same guest files to interact with the listed program IDs.

```sh
python3 scripts/fetch-release-programs.py
```

The fetcher accepts only the two named files from this repository's release. It
checks their exact sizes and SHA256 hashes before writing them, and refuses to
overwrite a file that has different contents. It does not execute an installer.

## Building from source

Use `scripts/build-rust.py` or `scripts/prepare-local.sh`. The helper normalizes
checkout and Cargo-cache paths before invoking the compiler. Two separate macOS
checkouts produced identical guest ELF files; the comparison is recorded in
`evidence/reproducibility/path-reproducibility.json`.

The Linux compiler distribution produced different guest image IDs. A source
build on Linux is therefore used for a fresh local deployment, not silently
substituted for an already-deployed testnet program. The public acceptance test
uses the published files explicitly and still verifies every byte against the
manifest before submitting a transaction.

## Public acceptance

The manual `Public testnet allowlist acceptance` workflow runs on standard
GitHub-hosted Ubuntu runners in this public repository. Each job creates its own
fresh test identities and distinct allowlist, then checks private claims against
the official testnet. No maintainer wallet or model API is used. Private logs and
wallet data remain inside the runner; only public transaction references and
counts are printed in the report.

The resulting report states which program files were used. Compilation alone is
not a successful acceptance test; the final report must show confirmed claims.


The public Actions workflow is a one-claim smoke test in each of two fresh
instances. Replaying ten sequential private proofs on a short-lived hosted
runner exceeded its five-hour deadline. Full-scale evidence is instead the
completed 20-claim laptop run under `evidence/testnet-completed.json`, independently
checked against public testnet. The standalone real-proof CI still exercises the
complete local workflows.

For a fresh ten-claim public run on persistent local hardware, use
`--claims 10 --timeout-seconds 43200`. Its report states the required count for
that run. A one-claim smoke pass is never reported as 20-claim acceptance.


## Post-reset deployment, 9 September 2026

`deployment.json` now points to the fresh distributions and approval group after
the 8 September reset. `deployment-pre-reset-20260907.json` retains the earlier
addresses. The program artifacts have not changed; their immutable hashes and
download location in `manifest.json` are intentionally preserved.

Use `python3 scripts/check-public-state.py` for current counts and execution state.
The pre-reset twenty-claim receipt remains historical. The later public read in
`evidence/current-state-20260911-1404.json` verifies both post-reset distributions
with ten unique claims each and the original policy at value 43, sequence 2.
The independently created native-UI policy reached value 125 in block 4437; its
receipt is `evidence/ui-execution-4437.json`. These are distinct instances.

`adapter.json` identifies the separate governed-setting consumer. Its public
deployment, initialization and application confirmed in blocks 4404, 4405 and
4406. It consumed the original executed value 43, sequence 2, not the newer UI
policy. `python3 scripts/check-policy-consumer.py` independently verifies its
current public state without a wallet or transaction.

See `docs/VERIFICATION.md` for the evidence boundaries, successful standalone
real-proof CI and fresh native package installation. Narrated videos and prize
acceptance are not implied by those observations.
