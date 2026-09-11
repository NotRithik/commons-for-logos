# Commons CLI contract

The native Basecamp backend launches an explicitly configured `commons-logos-cli` executable with `QProcess::setProgram()` and a fixed argument table. It never launches a shell. This document describes the current source contract; use a client and module from the same release.

## On-chain operations

| Operation identifier | Fixed CLI arguments | Required argument names |
| --- | --- | --- |
| `allowlist.create_distribution` | `primitives allowlist-create --json-stdin` | `state_account`, `root`, `member_count` |
| `allowlist.claim` | `primitives allowlist-claim --json-stdin` | `state_account`, `witness_file` |
| `allowlist.inspect_state` | `primitives allowlist-inspect --json-stdin` | `state_account` |
| `threshold.create_group` | `primitives threshold-create --json-stdin` | `state_account`, `root`, `member_count`, `threshold`, `initial_value` |
| `threshold.propose` | `primitives threshold-propose --json-stdin` | `state_account`, `witness_file`, `next_value` |
| `threshold.approve` | `primitives threshold-approve --json-stdin` | `state_account`, `witness_file` |
| `threshold.execute` | `primitives threshold-execute --json-stdin` | `state_account` |
| `threshold.inspect_state` | `primitives threshold-inspect --json-stdin` | `state_account` |

The request is one JSON object on standard input. Its top-level keys are exactly `schema_version`, `network`, `wallet_dir`, `operation`, and `arguments`. The schema version is `1`, the network is `testnet`, and `wallet_dir` must be an absolute, marked testnet-wallet directory. Unknown fields and unknown operations are rejected. The operation must match the fixed command-line verb.

`state_account` and `root` are 32-byte values written as **64 hexadecimal characters**, not 64-byte values. A supported optional `0x` prefix does not count toward those 64 characters. New UI-generated references use lowercase hex without a prefix. Counts are JSON integers from 1 through 256; the threshold cannot exceed the member count.

`initial_value` and `next_value` are signed 64-bit integers carried as decimal **strings**. This preserves their exact value in QML/JSON without floating-point rounding. The CLI checks their numeric range and canonical decimal syntax.

`witness_file` is an absolute path to one protected **Borsh-encoded `MemberWitness`**, not a JSON private-key document or a bundle of multiple members. It must resolve to a regular, owner-only file inside the configured wallet. Its member key must also exist in that wallet. Witness bytes must never appear in command-line arguments, stdout, public logs, or public transactions.

### Public-state review binding

A successful threshold read returns `state.fingerprint`, a 64-character digest identifying the exact public account state observed. The native review flow sends it as the optional `arguments.expected_state_fingerprint` for `threshold.propose`, `threshold.approve`, or `threshold.execute`. It is not permitted on unrelated operations.

The native backend refuses a review whose cached account/fingerprint no longer matches. The CLI compares the fingerprint again against the current public account before performing the operation. The private proof path also compares the public pre-state used by the wallet's proof preparation. A mismatch returns a safe error rather than silently approving a different proposal.

Older direct clients may omit this optional field, but the native reviewed UI supplies it. This is a **client-side reviewed-state check**, not an added consensus guard in the already-deployed guest. A race after preparation cannot be described as eliminated by this field alone.

### Execution identity and confirmation

Threshold execution is a public, signature-free relay once the program's threshold is satisfied. The client prepares a deterministic transaction identity from the proposal sequence and value, preserving the deployed `Execute` instruction prefix. Retries of one intent have the same identity; different intents do not reuse a legacy unit-instruction transaction hash.

The client saves its execution checkpoint before broadcasting, checks the returned transaction hash and confirmed transaction, and checks the exact public postcondition. Success requires the intended sequence/value, enough approvals, and `executed=true`. A receipt for an old proposal is not success. Pending execution reconciliation checks the saved intent before clearing its checkpoint.

This transport tag is not a new consensus-level expected-sequence check. See `cli/src/execution_intent.rs` for the boundary. Private operations use the pinned upstream prove-and-send API; do not claim that all private submission failures have an independently durable pre-broadcast checkpoint. An uncertain result must be investigated, not automatically repeated.

### Environment and read-only mode

The native transaction client forces `RISC0_DEV_MODE=0`, `RISC0_PROVER=ipc`, `RISC0_EXECUTOR=ipc`, and the testnet network. No hosted prover is selected. The wallet endpoint is constrained to the official testnet; an explicit developer-only environment switch permits the fixed local sequencer address.

A `.commons-readonly` marker permits public reads and rejects writes before opening private wallet storage or submitting a transaction. A key-free reader needs the marker, testnet configuration and program IDs; it does not need `storage.json` or a member key. A changed testnet protocol or wallet history ahead of the current chain produces a specific error; wallets are not silently rewound.

## Responses

Standard output contains one bounded JSON result. Dependency diagnostics are separated from that channel and are not echoed into the UI. Successful reads include `success=true`, operation, testnet endpoint/network, program ID, state account, observed block ID, and decoded public state. Successful writes additionally return their confirmed transaction hash and measured operation time.

An allowlist state contains `root`, `member_count`, and `claims_count`. A threshold state contains `root`, `member_count`, `threshold`, decimal-string `value`, numeric `sequence`, and either a null proposal or a proposal with `sequence`, decimal-string `next_value`, `approvals_count`, and `executed`.

Failures have `success=false` and `error` with a stable code and a safe message. The process exits unsuccessfully. The GUI never treats malformed output, a terminated process, or a timeout as a confirmed transaction. Upstream error context that might contain wallet paths or witness data is not displayed verbatim. See [ERRORS.md](../docs/ERRORS.md).

## Local enrollment and invitation operations

These use a separate fixed command family:

| Verb | Arguments object |
| --- | --- |
| `governance catalog --json-stdin` | `{}` |
| `governance create-identity --json-stdin` | `label` |
| `governance prepare-policy --json-stdin` | `title`, `field_name`, `initial_value`, `threshold`, `members` |
| `governance prepare-list --json-stdin` | `title`, `members` |
| `governance join-policy --json-stdin` | `invitation` |

Their top-level JSON keys are exactly `schema_version: 1`, absolute `home`, selected `profile` identifier (empty for a new identity), and `arguments`. A profile identifier is a generated local identifier, not a path. The native adapter does not accept arbitrary shell commands, network endpoints, or wallet paths from these verbs.

These operations do not publish, propose, approve, execute, transfer, fund, or request a faucet. Identity creation reads the testnet checkpoint for a newly generated wallet. Draft preparation and invitation import use local data. Successful setup results explicitly report `transactions_submitted: 0`; the GUI rejects a setup response that does not have this boundary.

An enrollment contains exactly `schema_version: 1`, a short `label`, `account_id`, and `salt`. The address and salt are 64-character hex. The organizer receives this enrollment, **not the member's private signing key**. Duplicate normalized account IDs cannot occupy two seats.

An invitation contains the pinned program ID, state account, membership root, workspace kind, title, field name, value type, initial value, member count, threshold, one member enrollment, leaf index, and sibling hashes. Its `kind` is `threshold` or `allowlist`; missing kind in an older record defaults to `threshold`. Threshold invitations use `value_type: "i64"`. List invitations use `value_type: "membership"`, field `membership`, initial value `"0"` and threshold `1` as canonical local metadata; those fields do not turn a membership list into a threshold-voting program.

The membership commitment is scoped to **both the program and the state account**. Import verifies the proof path and that the selected identity owns the invitation's member account. It derives the private witness locally. An existing conflicting policy or credential is not overwritten. The native `Enable my membership` convenience action selects only an invitation matching the selected profile's own enrollment.

Setup is bounded, uses a home-level lock plus wallet-level locks, and preserves existing files on conflicts. Policy files are local navigation metadata, not evidence that a draft was published. Opening a saved workspace performs a separate live read; publishing it requires its own reviewed on-chain operation.

## Source of truth

`cli/src/validation.rs` defines on-chain input validation; `sdk/schema.json` is its published JSON schema. `cli/src/governance.rs` defines the local onboarding documents and validation. `module/src/commons_primitives_ui.rep` and `module/src/governance_backend.inc` define the native UI boundary. The guest programs and SPEL interfaces remain independent of these local navigation documents.

## Progress diagnostics

The current client emits a small set of fixed `COMMONS_PROGRESS_V1` stage markers
on stderr: `syncing`, `proving`, `submitting`, `confirming`, and `syncing-result`.
The native SDK recognizes only complete exact markers and maps them to built-in
status text. Other dependency output is discarded; a retained marker line is
bounded to 128 bytes and the existing overall output limit remains in force.
No wallet path, member address, credential, or proof input is carried in these
markers. A marker does not authorize an action or prove success. The final JSON
receipt and its validation remain the confirmation boundary.

The private upstream call combines proof generation and submission; `proving`
covers that call, not a precise claim that every millisecond was proving. Once
submission returns, the client reports waiting for confirmation. No percentage
or completion time is invented. Older matching releases may display a coarser
busy message while following the same real-proof path.
