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
