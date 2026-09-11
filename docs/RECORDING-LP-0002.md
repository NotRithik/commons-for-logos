# LP-0002 recording plan: private M-of-N approvals

This is a recording plan, not a statement that an unrecorded or unverified step has passed. Rehearse against the submitted release. Keep the final source commit, matching client/module downloads, CI run and video together. The official basis is `logos-co/lambda-prize/prizes/LP-0002.md` and the repository's Evaluation Policies, retrieved on 11 September 2026.

## What the viewer must understand

Commons lets a group change a shared value only after enough eligible members approve. Members use shielded LEZ accounts; the public application state does not list who approved. This is a real parameter-change integration, not a treasury-transfer demonstration. LP-0002 explicitly permits a parameter change as its reference action.

The builder must narrate what was built and why, architecture and significant implementation decisions, and the complete private-approval/execution flow. Include real terminal output showing proof generation with `RISC0_DEV_MODE=0`. A silent screen capture, a progress timer, or a printed environment variable alone is not proof of a successful private transaction.

Aim for a clear 10-15 minute edited walkthrough rather than an hour of silent waiting. Keep the original footage. Cuts across long proving waits should be labelled; keep the same policy address, proposal sequence and intended value visible before and after the cut. Never splice unrelated receipts together as one execution.

## Before recording

Use the Commons source and release assets, not Kite's repository or release. The shared Commons module also contains LP-0003 membership registration, but this video's acceptance flow is the threshold program. Keep the module installed in its own Basecamp profile. See [QUICK-START.md](QUICK-START.md) and [NATIVE-BUILD.md](NATIVE-BUILD.md).

Have three distinguishable demo member identities and a 2-of-3 policy ready. For a new rehearsal, use a recognisable name such as Community grant limit, setting grant_limit_units, starting value 100 and proposed value 125. Do not reuse these values as an excuse to recreate an existing in-flight policy. Load its recorded state and continue from there.

Complete enrollment exchange off camera. Each participant supplies enrollment, not a signing key. The organizer prepares the policy and supplies an individual invitation. Each member imports their own invitation; an organizer who is also a member uses Enable my membership. A saved draft is not an on-chain deployment.

Have a clean terminal with the real local-demo output available, the repository open at its final commit, the default-branch CI result, the standalone-proof result, and the public deployment evidence. Do not expose wallet storage, `.env`, private credential files, inner journals, or proof inputs. Show only public receipts and the runner's sanitized report.

Check that no existing proof is running before starting another rehearsal or restarting an app. Do not repeat a completed action to recover a recording. A read-only state refresh is the recovery step.

## Recording flow and narration

### 1. Problem and outcome

Show Commons inside Logos Basecamp, then the named policy.

Suggested narration: "This is Commons, a native Basecamp application for private group approvals. The example is a group changing its grant limit. Two of three members must agree. The proposed value is public, but the program does not publish the approving members' addresses."

Explain that this is testnet software and is not audited custody software. Avoid claiming an actual grant was paid.

### 2. Understandable setup

Open My workspaces and the policy's saved details. Show the member count, required approvals, setting name and initial value. Briefly show the create-policy form without accidentally preparing or publishing a duplicate. Use names rather than reading long hashes aloud.

Suggested narration: "An identity is a member wallet stored on this device. A policy is a separate shared record. Its blockchain reference is like the address of a shared document; knowing that address does not give voting rights. I choose the members and threshold, prepare a local draft, then review publication separately."

When demonstrating one laptop with several identities, say: "These are separate synthetic wallets on one laptop for the demo. In a real group, each member creates and keeps their own keys on their own device. This recording does not demonstrate anonymity from the person controlling this laptop."

### 3. First publication and live read

For a genuinely new recording run, review and publish the prepared draft once. Show the confirmed public block and read the live state. For an already published run, explicitly say it was published earlier and show its existing receipt; do not label the refresh a deployment.

Point out the current value, the 2-of-3 rule, and that no proposal is waiting. Use Blockchain reference (advanced) briefly to show the exact policy being followed.

### 4. Private proposal

As the first member, enter the new value and open the review. Explain the exact change before confirming. Show the real proof status and, in the terminal segment, the real local proving path. Wait for the confirmed result or use an explicitly labelled edit.

Suggested narration: "Proposing suggests 125; it does not change the current value and it does not automatically cast my approval. The client generates a real proof locally. After confirmation, the current value is still 100 and the proposal has zero approvals."

Show that a second proposal and execution are unavailable while this proposal is pending. Do not describe a disabled button alone as proof of the consensus rule; explain that the program also checks the threshold.

### 5. First approval and restart recovery

Review the exact proposal as the first member and approve once. After confirmation, show one of two approvals and that the value has not changed. Do not quit while proof generation or confirmation is still in progress.

Now close and reopen the idle Commons window, select the existing identity and policy, and refresh. Show that the partial approval remains. Do not recreate the wallet or policy.

Suggested narration: "The first approval is stored on chain, not just in this window. Reopening the client does not lose it. One approval is still insufficient, so the change cannot be applied."

Demonstrate a duplicate approval rejection only after the original approval is confirmed. Show the understandable error and refresh to show the count is unchanged. Distinguish a locally rejected request from an included chain transaction; both must not be described as a second vote.

### 6. Independent second approval and execution

Select the second member's identity and the same named policy. Show the proposed value and the approval review, then approve once. Explain that switching identities itself does not sign or vote, and the second member uses their own private credential.

After confirmation, show two of two required approvals and that Apply change / Execute is now available. Review and apply the decision. Inspect the result: the same proposal sequence must be marked executed, and the current value must equal the intended new value.

Suggested narration: "The final relay is public. It does not need the organizer's private signing key; the program authorizes it from the recorded threshold. The client checks the exact resulting proposal and value, not merely that some old transaction hash exists."

Show repeat execution disabled, the confirmed block and transaction hash. This is the central end-to-end outcome.

### 7. Architecture and the important choices

Use the source tree and these four layers: Basecamp QML screen; native Qt/SDK boundary; local Rust client and RISC Zero prover; LEZ program and public policy state. Show the SPEL-generated threshold IDL and the program ID from the release ledger. See [PROTOCOL.md](PROTOCOL.md), [PRIVACY.md](PRIVACY.md), and [CLI-CONTRACT.md](../sdk/CLI-CONTRACT.md).

Explain that this design uses private membership proofs and per-proposal nullifiers, not FROST signatures or a claim that the member list is public. The eligibility commitment binds membership to the program and policy. The nullifier prevents the same member approving the same proposal twice without storing their member address in the public approval list.

Explain the LEZ account-model choice: the application does not demand a fresh zero-nonce public signing key for every approval. Fresh default private member accounts are initialized on first use; initialized members retain their owner, balance and data. The wallet follows the private account nonce lifecycle. Reusing a member after its first private action is relevant evidence, not an optional cosmetic step.

Discuss the execution-identity correction honestly: different proposal executions must not resolve to an old identical public transaction hash. The client binds its prepared transport identity and verifies the intended postcondition. The reviewed-state fingerprint is a client-side safeguard; it is not a new consensus-level expected-sequence instruction in the deployed guest.

### 8. Reproducibility, limitations and closing evidence

Show real standalone proof output from the submitted revision. From a clean prepared clone the documented path is:

```sh
/bin/sh scripts/prepare-local.sh fetch
/bin/sh scripts/prepare-local.sh build
python3 scripts/demo-local.py --mode threshold
```

Do not launch these commands again while their original run is active. The runner's real output identifies local IPC proving and `RISC0_DEV_MODE=0`; its final sanitized report contains the observed result and accepted receipts. Show the start/proving output and completed report, not only a green icon. Clearly distinguish standalone-local evidence from public-testnet evidence.

Show default-branch CI and the standalone-proof run for the final revision, the public deployment evidence, and the downloadable matching module/client checksums. Open [PERFORMANCE.md](PERFORMANCE.md): distinguish measured guest cycles from proof-plus-submission wall time, and disclose that the pinned RPC does not report a billed-CU value. Do not invent a conversion to gas.

Close with the privacy boundary: public proposal contents, counts and timing remain visible; traffic correlation, compromised devices and an operator who possesses all demo keys are not hidden by this application. Wallet files are permission-restricted, not encrypted custody storage. The fixed-member reference policy has no proposal expiry/replacement governance. These are limitations, not features to conceal.

## Final recording check

The video must contain your audible explanation, native Basecamp interaction, a shielded proposal and distinct approvals, below-threshold refusal, exact executed outcome, actual real-proof terminal evidence, architecture, account-model compatibility, and limitations. Include the restart/partial-approval and duplicate-vote observations in the video or tightly linked supporting material. Confirm every link and final commit before using the separate LP-0002 solution template. The builder must personally confirm eligibility, code rights and agreement to the prize terms before submission.

## Additional segment: another contract uses the approved decision

Show this after the complete private M-of-N flow, not instead of it. Open
`docs/COMPOSABILITY.md` and the separate consumer program's source. Explain:

> The private approval primitive does not have to implement every application's
> business rules. This second LEZ program owns a different state account. It reads
> the authenticated public policy account, checks the expected program and policy,
> requires the current decision to have executed, and consumes that sequence once.
> It then updates its own setting. No member key or private witness is handed to it.

Run `python3 scripts/check-policy-consumer.py` from the submitted checkout.
Show its **live** consumer value, consumed sequence, policy reference, owner-program
checks and block range. Use the existing completed consumer rather than redeploying
or repeating a private approval for this segment. In the recorded public run the
separate consumer adopted value43, sequence2; this is the earlier reference policy,
not the later UI demonstration of100 to125. Name that distinction on screen.

Show the actual saved consumer deployment/initialization/application receipts
and the exact guest replay refusal. Say that replay was refused by the compiled
guest in a local execution check; do not call it a second rejected network
transaction. The successful consumer application itself was confirmed on testnet.

> This is account-state composition, not an automatic callback or an arbitrary
> cross-program call router. A consuming application must define what the approved
> value means and bind its target and replay rules. Mapping1 and2 to application
> choices does not make this a yes/no/abstain secret-ballot election: members still
> approve one exact proposed outcome under the M-of-N rule.

The private-proof terminal footage is still required for the preceding approval
flow. This additional public consumer transaction is not a private proof and must
not be presented as one.
