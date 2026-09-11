# Error Review

The primitive exposes deterministic application errors through the `Error` enum. Guest programs currently panic with the formatted error string on application failure. The host tests check stable formatting for at least one error.

The current codes observed in `testnet/crates/primitives/src/lib.rs` are:

| Code | Name | Meaning |
| --- | --- | --- |
| 1001 | `AccountCount` | Wrong number of input accounts for the instruction. |
| 1002 | `Unauthorized` | Required authorized account metadata was absent. |
| 1003 | `AlreadyInitialized` | Create used an account that is not default-owned, has data, or has nonzero nonce. |
| 1004 | `WrongOwner` | Existing state account is not owned by this program. |
| 1005 | `InvalidState` | Deserialized state had wrong magic, invalid Borsh, or impossible shape. |
| 1006 | `InvalidSize` | Member count is zero or above the supported limit, or proof length exceeds the supported depth. |
| 1007 | `IdentityMismatch` | Witness identity did not bind to the authorized member account. |
| 1008 | `InvalidMembership` | Entitlement is zero, Merkle proof is invalid, proof length is wrong, or member index is invalid. |
| 1009 | `DuplicateClaim` | Distribution nullifier was already used. |
| 1010 | `CapacityReached` | Distribution already has as many claims as member count. |
| 1011 | `InvalidThreshold` | Threshold is zero or greater than member count. |
| 1012 | `PendingProposal` | A new proposal was attempted before the current one was executed. |
| 1013 | `NoProposal` | Approval or execution was attempted without a proposal. |
| 1014 | `DuplicateApproval` | The same proposal nullifier was already used. |
| 1015 | `ThresholdNotMet` | Execution was attempted before enough approvals. |
| 1016 | `AlreadyExecuted` | The proposal was already executed. |
| 1017 | `ThresholdAlreadyMet` | Additional approval was attempted after the threshold was met. |
| 1018 | `SequenceOverflow` | Proposal sequence increment overflowed. |
| 1019 | `DataTooLarge` | Serialized state did not fit LEZ account data. |
| 1020 | `InvalidViewingPublicKey` | The v0.2.4 private viewing key has an invalid shape. |
| 1021 | `UninitializedMember` | A default-owner member already has nondefault state and cannot be safely reused. |

## Review Notes

- The errors are precise enough for clients to distinguish eligibility failures, duplicate-use failures, threshold lifecycle failures, ownership failures, and state corruption.
- `InvalidMembership` intentionally covers several proof-related cases. That avoids revealing exactly which part of a Merkle witness failed, but clients should provide local preflight diagnostics before proof generation.
- `IdentityMismatch` is distinct from `InvalidMembership`, which is useful for client integration bugs involving account derivation.
- Public transaction failure semantics belong partly to LEZ settlement. Public execution failures may still be chargeable, even if the primitive itself does not return an app-state diff.
- Error strings should not include witness fields. Current formatting uses only code and variant name.

## Native client errors and recovery

These stable client categories are separate from the deterministic program errors
above. They do not change the guest wire format or program ID. Messages are fixed
strings: underlying errors may contain private data and are not shown verbatim.

| Client code | User action and boundary |
| --- | --- |
| `LOCAL_PROVER_UNAVAILABLE` | An explicitly configured absolute proof-engine path is missing or not executable. Restore the matching local engine. This attempt has not opened a wallet or submitted a transaction. Unconfigured/PATH discovery remains the upstream prover's responsibility. |
| `LOCAL_PROOF_FAILED` | The pinned wallet returned its typed circuit-proving error before sending this transaction. Check local proof dependencies, then refresh and review again. This classification is not applied to arbitrary send errors. |
| `MEMBERSHIP_CREDENTIAL_UNREADABLE` | Select the correct protected credential or re-import the original invitation for this identity. A malformed credential does not count as a claim or approval. |
| `WALLET_BUSY` | Wait for the other operation using this member wallet. Do not remove lock files or start a second copy. |
| `REVIEWED_STATE_CHANGED` | Refresh and review the current proposal. The previous reviewed state is no longer current. |
| `READ_ONLY_WORKSPACE` | Use your own member identity for a write; a key-free viewer cannot register or approve. |
| `EXECUTION_RECONCILIATION_REQUIRED` | Preserve the existing pending intent and reconcile its exact result; do not repeat an execution whose outcome is unresolved. |
| `CLI_REQUEST_FAILED` | An unclassified request failed. Inspect the actual current state and any pending transaction before retrying. This generic error makes no claim that a transaction was not submitted. |

Network/version/profile errors retain the specific categories in
`cli/src/main.rs`. A rejected local request, a failed local proof, an included
failed transaction and a lost response after submission are different outcomes.
The UI never advances a count merely because a request was accepted or a timer
is running. Only the confirmed result/live public state determines success.
