# Use private approvals from another LEZ program

Commons has two distinct integration boundaries. The Basecamp/Qt SDK helps a user create, inspect and interact with a policy. The Rust **policy adapter** is guest-side code for another LEZ program to consume an authenticated decision. A host-side SDK alone is not evidence that a second contract enforces the policy.

The reference consumer is `testnet/crates/adapter/examples/governed_setting.rs`. It is a separate compiled LEZ program with its own state account. It does not modify or replace the deployed threshold program, and it does not receive a member's invitation, secret key or approval witness.

## Contract-level flow

A group first completes the ordinary Commons flow: propose a value, obtain M distinct private approvals, and execute the decision. A consumer transaction then includes two **actual LEZ input accounts**: its own setting account and the public Commons policy account. The consumer checks the policy account's identity, owner program, decoded state, executed proposal, threshold and sequence before changing its own state.

The policy state is supplied by LEZ as an account input, not as an unauthenticated JSON object downloaded from an arbitrary website. A caller cannot substitute an invented account with the same bytes: the saved account ID and owner program must both match. The foreign policy account's application data, owner and balance are returned unchanged.

This is account-state composition. It is **not** a general cross-program call router, an automatic callback, or a promise that an executed decision will invoke an arbitrary recipient contract. Someone must submit the separate consumer transaction. After policy execution, that transaction can be submitted without a member signing key.

## Guest-side API

Add `commons-logos-policy-adapter` from `testnet/crates/adapter` to the consumer guest. The relevant interfaces are:

- `PolicyBinding`: the trusted threshold program ID and the exact policy account ID.
- The internal `read_policy` helper validates and decodes that bound policy account; use the public executed-decision helpers for authorization.
- `read_executed_decision`: additionally requires a current executed proposal with sufficient distinct approvals and matching sequence/value.
- `consume_after`: additionally requires a sequence greater than the consumer's saved last-consumed sequence.

The binding must come from immutable configuration or the consumer's own protected state. Do not let each apply caller select any policy address they like. Otherwise an attacker can create an unrelated one-member group and authorize their own action.

For a one-time effect, store the accepted sequence together with the effect in the same successful state transition. Do not write a local flag or trust a front-end counter. A failed consumer execution must leave both the effect and that sequence unchanged.

The reference `GovernedSetting` state stores a binding, integer bounds, a value, and `last_consumed_sequence`. `Initialize` requires an authorized unused consumer account and fixes its binding and bounds. `Apply` accepts an exact expected sequence and value, checks them inside the consumer guest, and updates that consumer once. A duplicate, older, wrong-policy, unexecuted, mismatched or out-of-bounds decision fails.

## What an integer can and cannot authorize

The underlying reference policy controls a signed 64-bit integer. The group name, setting name and units shown in Basecamp are local labels. The threshold guest does not interpret `grant_limit_units` as a currency or `storage_limit_gb` as an instruction to provision storage.

A different consumer can give the approved integer a precise application meaning. For example, a feature-setting contract can define `1 = enable`, `2 = disable`, and `3 = retain the existing setting`. That consumer must reject other values and pin the feature, target and policy in its own trusted state. The current reference consumer demonstrates a bounded integer setting, not this application-specific feature contract.

An important distinction: this remains **M-of-N authorization of one proposed outcome**. It is not a secret-ballot election with independent yes/no/abstain tallies, ranked choices or competing simultaneous proposals. Approval means agreement to the exact proposed integer. Mapping a number to an enum does not change the voting protocol into a general election system.

Do not use an unbound caller-supplied action table such as `42 means send all funds to this address`. For richer operations, a consumer needs immutable action definitions, a separately committed action registry, or an appropriate proposal payload commitment. Its policy must bind the target, operation, parameters and replay domain. Those are additional application responsibilities, not guarantees supplied by a display label.

## Freshness, replay and liveness

The adapter consumes the **current executed proposal**, not merely the account's retained value. Starting a new proposal leaves the old value in state, but the adapter does not mistake that retained value for a newly executed authorization. A new, still-unexecuted proposal therefore blocks consumption through `read_executed_decision` until the new proposal executes.

The threshold reference stores the current proposal rather than a permanent history of all completed proposals. A consumer that needs every historical event must arrange its workflow so each relevant decision is consumed before it is superseded, or add a separate history/commitment design. The adapter must not silently skip a required business action just because a later sequence exists.

The reference consumer uses strictly increasing sequences per bound policy. This allows it to follow the newest executed setting; it does not require every intermediate sequence. Applications needing contiguous processing should require `sequence == last_consumed_sequence + 1` and define an explicit recovery process.

Expected value and sequence are checked **inside the consumer guest**. The deployed threshold program predates a consensus-level expected-sequence argument for `Execute`; its CLI's transport tag and reviewed-state check are separate safeguards, described in `PROTOCOL.md`. Do not confuse those client protections with the consumer's own on-chain checks.

## Privacy boundary

The consumer learns the public decision, policy reference, member count, threshold, proposal sequence and public activity information. It does not require member account addresses, signing keys or private witnesses. This does not hide transaction timing, a known off-chain correspondence with a person, or information revealed by a compromised member device. The original privacy model in `PRIVACY.md` still applies.

The consumer transaction and its output are public. It does not itself generate a private approval proof. A public consumer receipt is evidence of on-chain composition, not new evidence of private proof generation or of twenty allowlist claims.

## Build the separate guest

Use the pinned prerequisites and compiler from `LOCAL-DEMO.md`. With `GUEST_RUSTC` pointing to the verified RISC Zero guest compiler:

```sh
python3 scripts/build-rust.py guest \
  --manifest testnet/Cargo.toml \
  --target-dir out/adapter-guest \
  --guest-rustc "$GUEST_RUSTC" \
  --package commons-logos-policy-adapter \
  --example commons_governed_setting

cargo +1.94.0 run --locked --offline \
  --manifest-path testnet/Cargo.toml \
  -p commons-logos-testnet-sdk --bin pack_v024 -- \
  out/adapter-guest/riscv32im-risc0-zkvm-elf/release/examples/commons_governed_setting \
  out/commons_governed_setting
```

The example has its own image ID. It must never be presented as the original threshold or allowlist guest artifact. Keep the original release program hashes intact.

## Real public-network runner

`integration/src/bin/policy_adapter.rs` is the separate reference runner. It accepts an already executed policy, exact sequence and value. It creates an isolated public-only wallet and one consumer account, deploys the adapter, initializes its binding, and applies the decision. It does not load the group members' wallets, request faucet funds, transfer tokens, or repeat their private approvals.

The command form is:

```text
policy_adapter run ABSOLUTE_NEW_DIRECTORY PACKED_GUEST POLICY_BASE58 SEQUENCE VALUE
```

It requires `COMMONS_ALLOW_PUBLIC_TESTNET=1` and `RISC0_DEV_MODE=0`, and uses only the official testnet endpoint. Read the live policy before supplying the expected sequence/value. Build the binary with the pinned integration manifest and dependencies before use. The directory's parent must already exist.

Each outgoing stage records an intent before sending, then records its returned hash before polling for confirmation. Reusing the same directory and exact intent reconciles known hashes rather than sending them again. An intent with no returned hash is uncertain: inspect it and the chain instead of deleting the record or generating another wallet. A crash during first-time wallet preparation also preserves that incomplete wallet for inspection rather than replacing its keys.

`public-events.jsonl` and `public-report.json` contain the public results. A guest execution measurement is explicitly marked `proof: false`; an actual chain confirmation records its transaction hash and block. The final report requires the exact consumer value and sequence, the expected consumer owner, unchanged foreign policy application state, and replay refusal in the actual compiled guest. The replay check does not submit another transaction and must not be described as a second network rejection.

A successful build alone is not deployment evidence. Consult the actual sanitized report and the release verification ledger for which network run completed, against which image ID and source revision. This document does not certify an unobserved run.

## Independently inspect the published consumer

After the release's `release/adapter.json` is present, run:

```sh
python3 scripts/check-policy-consumer.py
```

This command loads no wallet and submits no transaction. It independently decodes
the actual public consumer account, checks its owner and policy binding, and
compares the consumed sequence and value with both the release record and the
currently executed threshold policy. A changed or reset network produces a failed
match rather than a fabricated historical success. Read the block range and
observation timestamp. Historical replay refusal and proof timings are separate
recorded evidence, not facts that this single current-state read can establish.

The separate consumer SPEL interface is generated from its actual enum and state
in `idl/src/adapter.rs`, using the pinned official SPEL parser. Run the same IDL
command documented for the other programs; its outputs include
`idl/generated/commons_governed_setting_v024.json`. The consumer has two public
accounts: its own writable state and the read-only foreign policy. Its instruction
arguments use the LEZ v0.2.4 RISC Zero word encoding; its persistent state uses
Borsh. A generated declaration file describes the interface and is not a second
executable implementation of the program.
