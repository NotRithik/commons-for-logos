# Canonical public-testnet protocol

## Version boundary

The public implementation is `testnet/crates/*`, pinned to LEZ v0.2.4 revision `47eba256479f6f785acbd138834340703cd03401`. Its host SDK serializes guest instructions with **`risc0_zkvm::serde::to_vec`**. Persistent application state and the small local witness files use **Borsh**. These are different boundaries; do not interchange their codecs.

The root `crates/*` experiment targets the later LEZ development ABI and is not the public deployment. Its input envelope must not be used for a v0.2.4 transaction. The public CLI checks the deployed protocol fingerprint before executing work.

## Instructions and state

`DistributionInstruction` has `Create { root, member_count }` and `Claim { witness }`. Its state contains the root, member count and the list of accepted application nullifiers.

`GroupInstruction` has `Create { root, member_count, threshold, initial_value }`, `Propose { witness, next_value }`, `Approve { witness }` and `Execute`. Group state contains the root, member count, threshold, current integer value, sequence and optional proposal. A proposal contains its sequence, proposed value, approval nullifiers and an executed flag.

Account counts are exact: create and execute use one state account; claim/propose/approve use the state account plus one member account. Existing state must be owned by this exact program. Fresh state creation requires a default, authorized account; existing state cannot be reinitialized.

## Commitment and uniqueness construction

Hashing uses SHA256 with explicit domain separation and little-endian 64-bit lengths before every variable part. The context commits to both the executing program identity and the state account ID. A member leaf commits to that context, the actual member account ID, a salt and a positive entitlement. Empty tree leaves have a distinct domain. The canonical padded binary tree supports 1 through 256 members and at most 8 sibling hashes; an index outside the original member count is rejected even if it fits a padded tree.

The application nullifier commits to context, purpose (`allowlist` or `threshold`), proposal sequence, member account ID and the member's nullifier secret. Before accepting it, the program derives the private-account identity from that secret, the viewing public key and identifier, and compares it with the **actual authorized LEZ input account**. A caller cannot buy a second claim by substituting a different nullifier secret.

A distribution stores each nullifier once. A proposal stores each approval nullifier once and can execute only after its configured threshold. Nullifiers are distinct across contexts and proposal sequences. Repeated execution is rejected.

## Fresh private accounts and nonce rotation

In v0.2.4, every private account use rotates its nonce even when balance and data do not change. Leaving a brand-new member in the default-owner state after this rotation makes it invalid on its next use.

The corrected program explicitly requests `Claim::Authorized` when first using an authorized, entirely default member account. The protocol assigns its program owner before nonce rotation. The program does **not** change the member's balance or data. For an existing initialized member, the owner, balance and data are preserved. A malformed legacy member with default owner but already nondefault state is rejected with `UninitializedMember` (1021), not silently reused.

The reference membership identities should be dedicated zero-value accounts. This is not a general-purpose asset wallet, a recovery program, or a migration tool for malformed accounts. The tests include actual protocol validation plus realistic private-nonce transitions, not only application-state counters.

## Private versus public execution

Create and threshold execute contain no membership witness and use public execution. Claim, propose and approve **must** use the local LEZ private proving path. Their witness-bearing input bytes and inner program receipt/journal are not public artifacts. The outer privacy proof binds the authorized account transition without publishing those inputs.

The production CLI cannot select a public path for a witness-bearing operation. It requires `RISC0_DEV_MODE=0` and local IPC proving. Optional local VM preflight measures aggregate cycle counts but does not establish a completed proof or confirmed chain transaction.

## Authorization and recovery

Threshold execution is permissionless **after** the threshold has been met; the production CLI uses an unsigned public state-account reference for this operation. An authorized creator is still required to initialize that state account.

The CLI takes an exclusive per-wallet lock. After a transaction is broadcast, it atomically persists the hash and operation before awaiting finality. An interrupted client must reconcile that pending hash and resynchronize wallet state before attempting a different mutation. A network error is not taken as proof that an earlier transaction never landed.

## Evidence boundary

Host tests, guest compilation, VM preflight cycles, private proof generation, sequencer acceptance, and public testnet finality are separately recorded stages. No stage is inferred from the previous one. Current receipts and remaining gates are listed in `evidence/verification.json`.
