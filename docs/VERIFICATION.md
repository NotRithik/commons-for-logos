# What was actually verified

This ledger distinguishes real network outcomes, native user-interface observations,
compiled guest execution, and fixture checks. It is not a claim of prize acceptance.
The native module and two original programs are Commons for Logos; Kite / LP-0008
is a separate repository, module and release.

## Private governance: the new native-UI instance

A three-member policy started at 100 and required two distinct private approvals.
The policy was published in block 4278. The proposal confirmed in block 4325,
the first approval in 4362, the independent second approval in 4426, and the
execution in 4437. The final value is 125, sequence 1, executed=true.
`evidence/ui-execution-4437.json` records the actual final UI receipt.

The first approval remained present after the client was restarted while idle.
A deliberate duplicate approval was refused with an already-approved message.
The UI refused execution before the threshold, and disabled repeated approval
and execution after completion. This is parameter control, not a treasury transfer.
Original actor labels are local demo names, not public on-chain voter identities.

## Private membership

`evidence/current-state-20260911-1404.json` independently verifies ten unique
accepted claims in each of two post-reset distributions. The newer native-UI
membership example is a third, separate list with three eligible members.
Its first private registration confirmed in block 4430. A duplicate registration
was refused, and a later native read retained exactly one registered member.
Twenty unique nullifiers are not a claim about twenty independently verified humans.

## A separate on-chain consumer

`release/adapter.json` pins the consumer image, its account and the exact original
policy it trusts. `evidence/adapter-completed-20260911.json` contains its public
receipts and precise scope. Deploy/initialize/apply confirmed at 4404/4405/4406.
The consumer adopted value 43 at sequence 2 and retained the foreign policy's
application state. No private approval was replayed and no member wallet was opened.

The real compiled guest refused duplicate consumption with error 2006. That check
was not an additional network transaction. Guest execution and its cycle count are
explicitly marked proof=false. The independent read in
`evidence/adapter-current-20260911-1623.json` matched the actual consumer and
executed policy at block 4479. That read cannot establish historical proof privacy
or substitute for the recorded deployment transactions.

## Native installation from the public package

The published v0.2.0-rc.1 module was installed through Basecamp's Package Manager
into a fresh isolated profile. The app opened in view-only mode, with no member
wallet, and read the newer membership list at block 4481. Registration remained
disabled. `evidence/clean-install-20260911.json` records that observation.
The downloaded package's native libraries and QML also matched the installation
used for the completed real governance flow. Compilation alone was not used to
mark this GUI workflow successful.

## Standalone real-proof CI

The original run [34309398475](https://github.com/NotRithik/commons-for-logos/actions/runs/34309398475)
completed successfully. Its real-threshold job completed on 11 September 2026;
its real-allowlist-smoke job had completed on 9 September. The runner uses
RISC0_DEV_MODE=0 and local IPC proving, not mock proofs or a hosted proving service.

The run's commit is be635758f3d5985a7a2d2f32dbb272255972c67e. The v0.2.0-rc.1
release commit is 42f3bd035209099718ce84ee9ccfc70494b946ca. Their executed core
workflow, standalone runner, primitive and guest sources are byte-identical;
`evidence/real-proof-source-correspondence.json` lists the ten checked files and
hashes. This does not assert that the old job ran every new GUI or adapter feature.
Those have the separately identified evidence above. A smoke test with one claim
per distribution is not substituted for the separate twenty-claim public evidence.

Ordinary default-branch tooling and portable/native CI are separate jobs. Use their
exact commit-specific links; do not relabel an ancestor run as a newer full-tree run.

## Remaining submission boundaries

The builder must record and narrate the end-to-end demonstration, explain the
architecture and show actual proof-generation terminal output. Saved receipts and
this document do not create that recording. Clearly label historical receipts,
local sequencer output, public-testnet footage and cuts across long proof waits.

The pinned RPC exposes no per-transaction billed-CU receipt. `PERFORMANCE.md`
reports reproducible guest user cycles and separate real elapsed timings, and
links upstream issue 840. There is no claimed billed-CU conversion or maintainer
waiver. A reviewer must evaluate that explicit platform limitation against the
prize wording; it has not been silently marked an observed metered cost.
