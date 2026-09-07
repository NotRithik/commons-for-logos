# Privacy model and limitations

This is a testnet reference implementation, not an audit or a guarantee of production safety. Read this together with `PROTOCOL.md` and the pinned LEZ source.

## What the application tries to hide

For a witness-bearing claim or vote submitted through the required private proving path, the on-chain observer learns that a valid authorized member satisfied the committed eligibility rules, but not which leaf/account supplied the witness. The observer sees the public root, group/distribution state, action timing, nullifier count, and the public proposal parameter when applicable. Application nullifiers prevent repeated use without publishing a member-address list.

This claim is relative to an observer who does not already possess a member's spending/nullifier secret, witness, private wallet snapshot, proof input, or the off-chain correspondence between a recorded action and a person. SHA256 binding/hiding assumptions, LEZ private-transaction correctness and the RISC0 proving system remain dependencies.

## What the distributor or coordinator learns

A distributor that constructs an eligibility list necessarily knows the input addresses and salts it used. That does not by itself disclose a claim-time nullifier whose secret is controlled by a member. However, any coordinator that also generated or retained members' keys can link their actions. The automated demonstration harness generates controlled test identities and therefore is **not** a production key ceremony or evidence that its coordinator is ignorant of those keys.

The GUI demonstration supports separate member-only wallet profiles containing exactly one member key and one shared account, not all other members' keys or the creator's signing key. This tests software isolation and the ability to approve independently; it does not retroactively make centrally generated demo keys unknown to their generator. Real use requires members to generate and control their own keys and deliver the needed eligibility commitments through an appropriate off-chain process.

## What is not hidden

Network addresses, message timing, local logs, file names, transaction sizes, traffic correlation, known identities outside the chain, and all metadata exposed by the pinned underlying LEZ implementation are outside this application's anonymity claim. A small eligibility set may be inferred from outside information. The proposal's content is intentionally public.

A malicious/instrumented local client or a machine compromise can see a witness before proof generation. The CLI protects its interface and avoids intentionally printing secrets; it is not a defense against a compromised operating system.

## Secrets and storage

`MemberWitness` is not formatted with `Debug`. Witness-bearing instructions are never intended for public transaction submission. Inner program receipts/journals contain private material and must not be published; only approved public metadata and final outer transaction receipts should be shared.

**The pinned upstream wallet persists plaintext JSON.** This project uses dedicated wallet directories, restrictive permissions, no-follow file handling, maximum input sizes and one-writer locks. Those measures are not encryption. Protect disk access and do not use shared or attacker-writable directories. Never upload `runtime/`, wallet storage, private witness files, mnemonics, local proof inputs or inner journals.

The CLI and native process boundary strip hosted-prover credentials, force real local IPC proving and restrict endpoints to the intended testnet or explicit standalone loopback configuration. They do not require any LLM/model API, paid proving provider or custodian.

## Important fresh-account behavior

A fresh zero-value member account is explicitly claimed by the program on first private use so that the v0.2.4 nonce rotation does not leave it malformed. Existing initialized members retain their owner, balance and data. Use dedicated membership identities; this program does not implement general asset custody or account recovery.

## Failure and denial-of-service boundaries

Invalid membership, wrong identity, duplicate use, threshold errors and invalid state return deterministic application errors and do not produce application-state changes. Chain-level inclusion/finality failures remain separate; the CLI records pending transaction hashes and reconciles them before retrying.

Proof generation may take substantial time and memory on a laptop. The GUI reports an in-progress operation rather than pretending a proof has confirmed. Cancellation terminates the owned child-process group; an already-broadcast transaction can still land, so the next operation must reconcile it. No irreversible success or payout is inferred from a progress message.

A proposer can leave an active proposal without sufficient approvals. The reference program has no proposal expiration/replacement governance; this is a documented liveness limitation, not concealed as a production-complete DAO design.
