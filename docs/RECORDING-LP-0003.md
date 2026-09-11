# LP-0003 recording plan: private membership registration

This is a plan for the builder's narrated video, not an assertion that every staged feature has already passed. Use the exact source and matching client/module package submitted for LP-0003. The official basis is `logos-co/lambda-prize/prizes/LP-0003.md` and its Evaluation Policies, retrieved on 11 September 2026.

## The central story

A group publishes a commitment to eligible members. An eligible person registers once from a shielded LEZ account, without putting their membership address into public application state. The program prevents a second claim from the same member for that list. This reference integration is an allowlist membership gate, not a token-airdrop payout. The prize permits either reference integration; do not narrate a token transfer that the application does not perform.

The video must contain the builder's voice explaining what was built and why, the architecture and significant decisions, and a real private claim. Show terminal proof-generation output with `RISC0_DEV_MODE=0`. Also provide reproducible evidence for at least two public-testnet distributions and at least twenty unique claims across them. A fresh one-claim UI demonstration does not replace the twenty-claim evidence.

A clear 8-12 minute edited walkthrough is reasonable. Preserve original footage. Label cuts over long proving waits and keep the same list reference and claim outcome identifiable. The app's Save view / Record view facility captures frames only; it does not supply the required builder narration.

## Prepare before recording

Install the matching Commons LGX and companion client in a separate Basecamp profile. This is the shared Commons module, not Kite. The allowlist and threshold programs have different program IDs even though they share a desktop application and SDK. This video's claim is specifically for the allowlist program.

Use [QUICK-START.md](QUICK-START.md) for identity creation, enrollment exchange and invitations. Give the list a simple name. Exchange member enrollments and invitations off camera; they are not signing keys, but a public recording should not unnecessarily reveal the address-to-person mapping. Never display `storage.json`, private member credentials, `.env`, raw proof inputs or inner journals.

Prepare a member who has not already registered in the demonstration list. Do not delete a nullifier or recreate a wallet to make a completed claim look new. Keep the original two-distribution evidence separately available; a third fresh list may be used for the live UX demonstration without claiming it supplied the earlier twenty claims.

Check that no proof or uncertain transaction is still running before starting another action or restarting the app. Load existing identities after a restart; do not create duplicates to recover a slow request.

## Recording flow and narration

### 1. Explain the problem in ordinary language

Show the private membership screen in Logos Basecamp.

Suggested narration: "An ordinary public allowlist can expose the eligible addresses and which address claimed. Commons lets an eligible member register without publishing that address in application state. The chain still verifies eligibility and prevents a second registration. This example records membership; it does not send tokens."

State that this is a testnet reference implementation and not audited production software.

### 2. Show usable organizer setup

Open My workspaces. Show a named member identity and the Create membership list form. Show the list name and eligible members selected by enrollment. A local demo can select another locally created member by name; real participants send enrollment from their own device.

Suggested narration: "The organizer collects enrollment, not people's signing keys. The app builds the commitment and individual invitations. Preparing a draft is local; publication is a separate reviewed blockchain action."

Show the first-publication review and confirmed result for a new run, or explicitly identify an earlier publication receipt and read that existing list. A refresh must not be narrated as a new deployment. Point out the registration count and public list reference under Advanced.

### 3. Join as the recipient

Select the recipient's own identity and import their individual invitation. Show the successful local verification and saved membership state. A wrong-identity invitation should produce an understandable refusal without changing chain state. Use Enable my membership when the organizer is also an eligible member.

Suggested narration: "The invitation proves that my member account belongs to this exact list. The app checks it and derives the private credential locally. Importing it is not a claim. I still review and confirm registration separately."

For several synthetic wallets on one laptop, explain that this is a software demonstration of separate keys, not secrecy from that laptop's operator. In real use, each member controls their own device and key.

### 4. Make one real private claim

Open the named list, refresh its live status, and select Register privately. Read the workspace and acting identity in the review before confirming once. Show the real local proof-generation state and the terminal evidence. Allow the original request to finish; do not press again because it is slow.

After confirmation, show the block number, transaction hash, real-proof mode and updated registration count. Refresh again. Explain that the count increased by one but the public application state has not published a member-address list.

Suggested narration: "The proof demonstrates an authorized eligible member, without exposing which eligibility leaf supplied the witness to an on-chain observer who lacks the member's secrets and off-chain identity mapping. The resulting claim is recorded only after testnet confirmation."

Do not call a timer, request-accepted message or public progress marker proof of successful registration.

### 5. Show duplicate prevention and a recoverable failure

After the first claim is confirmed, attempt the same member's registration again. Show the specific already-registered refusal, then refresh and show that the public count did not increase. Explain whether the request was rejected by local preflight or by the sequencer. A preflight refusal is not an included transaction.

Demonstrate a wrong-list or malformed invitation/credential separately, without showing its private bytes. The error must be readable, the original list must not acquire another claim, and restoring the correct membership must remain possible. Do not deliberately interrupt an uncertain broadcast to manufacture a failure clip. A failed local proof and a lost response after submission are different cases.

Suggested narration: "A rejection does not count as a successful claim. The member can fix a genuine pre-submission problem and retry. When a submission's outcome is uncertain, the client must reconcile that transaction first; deleting a pending record is not recovery."

If this failure/retry sequence has not yet been observed on the submitted build, keep it marked pending in the evidence checklist rather than narrating it as tested.

### 6. Explain what is private and why

Show the source's four layers: native Basecamp view, typed Qt/SDK client boundary, local Rust/RISC Zero proving, and the LEZ allowlist program. Open the generated SPEL allowlist IDL and the release program ID. See [PROTOCOL.md](PROTOCOL.md), [PRIVACY.md](PRIVACY.md), and [CLI-CONTRACT.md](../sdk/CLI-CONTRACT.md).

Explain the membership commitment, its scope to both program and distribution account, the private membership proof and the per-list nullifier. A credential for one list does not grant membership in another. A member cannot choose a different nullifier secret to get another valid claim because the witness is bound to the actual authorized private account.

Compare with a public Merkle allowlist carefully: a Merkle root alone is not the privacy claim. The witness-bearing call goes through LEZ's private proof path; member addresses and the witness must not be sent as a public transaction. The organizer can know who supplied enrollment. An organizer or test operator who possesses member secrets is outside the claimed observer model.

Make residual leakage explicit: the root, list size, claim count, public list reference, transaction timing and other underlying network metadata remain observable. Off-chain coordination, a small set, traffic analysis or a compromised device may identify a participant. "Unlinkable" is relative to the stated threat model, not a promise of anonymity against every observer.

### 7. Show the two-distribution / twenty-claim evidence

Open the deployed program ledger and the timestamped completed public evidence. Show both distribution references, their distinct roots, each count, the combined count, and representative confirmed receipts. Explain that unique claims are scoped to the distribution; do not equate a claim count with independently verified unique humans.

The read-only verification entry point is:

```sh
python3 scripts/check-public-state.py
```

It inspects the release's public deployments without loading a member wallet or sending a transaction. Inspect the report it actually returns. A saved snapshot or earlier pre-reset receipt is historical evidence, not proof of the current chain state. If a reset has removed a deployment, disclose the difference instead of showing old counts as live.

The new single-claim UI list and the original twenty-claim evidence must be labelled distinctly. Do not rerun twenty costly proofs merely to regenerate a screenshot of a completed deployment.

### 8. Show clean reproduction, performance and final links

The standalone demonstration, after installing the documented prerequisites, is:

```sh
/bin/sh scripts/prepare-local.sh fetch
/bin/sh scripts/prepare-local.sh build
python3 scripts/demo-local.py --mode allowlist-smoke
```

This local smoke flow generates real proofs and claims in two local distributions. It is not the separate twenty-claim public-testnet acceptance record. Show its actual terminal stage output, `RISC0_DEV_MODE=0`, local IPC proving and the final sanitized report from the submitted revision. A printed variable or green unit-test job alone is insufficient.

Show the final default-branch CI result and standalone-proof result, matching downloadable module/client assets and checksums, SPEL IDL, public deployment evidence, and [PERFORMANCE.md](PERFORMANCE.md). Distinguish inner guest execution cycles from proof-plus-submission elapsed time and from an unavailable billed-CU RPC field; never invent gas numbers.

Close by acknowledging plaintext permission-restricted demo wallet storage, fixed eligibility after publication, proof latency, unhidden network metadata and lack of a production audit. Link the actual upstream issues for Logos problems encountered; do not invent or duplicate an issue. Complete the LP-0003 FURPS self-assessment using observed functionality, usability, reliability, performance and supportability.

## Submission check

The video needs audible builder narration, real Basecamp interaction, a real shielded claim and final confirmation, architecture/key decisions and precise privacy limitations. Supporting materials need the two distributions/twenty claims, reproducible real standalone demo, green final CI, error/failure evidence, benchmarks, SDK, SPEL IDL and matching downloads. Use a separate LP-0003 solution file and PR. The builder must personally confirm code rights, eligibility and agreement to the terms before submission.
