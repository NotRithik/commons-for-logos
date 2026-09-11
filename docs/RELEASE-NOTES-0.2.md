# Commons 0.2 release notes

This release contains one Commons Basecamp module and its separate native client. It does not include Kite / Commons Relay, member wallets, private credentials, a Qt SDK, or font files. LP-0002 and LP-0003 use different on-chain programs while sharing the Commons interface.

## Included

Named identities and workspaces; local enrollment and individually verified invitations; membership-list and integer-policy preparation; separate publication review; exact-value and reviewed-state checks; private proposal and approval flows; duplicate-use rejection; resumable public execution; visible invitation scrolling; compact task screens; and explicit local proof dependency setup.

The separate governed-setting reference program demonstrates checked on-chain composition. It authenticates the policy account and program, requires an executed decision, applies bounds, and records the consumed sequence with its own state update. It is not an arbitrary call router, token treasury, or secret-ballot yes/no election.

## Recorded verification

The new UI policy proposed 100 to 125, collected independent private approvals, and displayed the completed value 125 at block 4437. The new membership registration completed at block 4430; a duplicate was refused and a subsequent read remained at 1 of 3. Sanitized receipts and the manual observation ledger are in `evidence/`. The original two distributions and their 20 completed claims remain separate reference evidence.

The native package was checked using the official Logos LGX library and installed into an isolated directory. A modified private test copy was rejected. Native binaries are local developer builds, not notarized software. The LGX package is unsigned. No global system verification setting needs to be disabled.

## Operational limits

Use dedicated testnet-only identities. Wallet storage is plaintext protected by OS permissions, not encrypted custody. Local proofs can take many minutes. A pending operation is not success; reconcile an uncertain request before trying another action. There is no proposal expiry or replacement mechanism.

The existing threshold and allowlist image IDs remain pinned to the published program artifacts. A new host/UI release does not imply those programs were redeployed. Guest cycles and proof-plus-confirmation times are documented separately from billed compute units; the pinned RPC does not expose a billed-CU field.

## Submission boundary

A release is not a prize submission or acceptance. Builder-narrated video, applicable terms and eligibility confirmation, final CI links, and the separate solution pull requests remain submission steps. Consult the live verification record rather than treating this note as a blanket readiness claim.
