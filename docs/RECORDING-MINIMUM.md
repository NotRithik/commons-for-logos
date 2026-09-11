# Short recording plans

Both prizes require builder narration, the end-to-end flow, architecture and key implementation decisions, and actual proof-generation terminal output showing RISC0_DEV_MODE=0. The suggested length below is an editing target, not an official duration requirement. A silent screencast is insufficient.

## LP-0002: one complete group decision (about 4-6 minutes edited)

Show the named policy, its current value and the 2-of-3 rule. Explain that the policy is a shared on-chain record; membership is a separate shielded identity. Propose the new value, show the first private approval and the below-threshold state, then the second independent approval and the confirmed execution. Show the resulting value in a fresh state read. Include the real proof-terminal segment; label cuts over waits and keep the policy address and proposal sequence consistent.

Explain: a Merkle root commits the eligible set; a nullifier prevents the same member voting twice; the private proof hides which member acted. The executed example changes a setting, not a treasury balance. An additional short view of the separate governed-setting consumer shows how another program can use the executed decision. Its recorded value 43/sequence 2 belongs to the original policy; do not present it as consuming the newer 125 decision.

## LP-0003: one complete private registration (about 3-5 minutes edited)

Show the list commitment and eligible-member count, then the member's invitation and private registration. Include the real proof-terminal segment and confirmed receipt, refresh the list, and show that repeating the registration is refused. A read-only view of the two reference distributions shows ten accepted claims each; the video need not film twenty repetitions to explain those recorded counts.

Explain: the distributor knows who enrolled, the public chain sees the commitment and count, and the private proof hides which eligible account made this claim. The account-bound nullifier prevents another claim for that distribution. This is membership registration, not a token payout or token-balance check.

## Use the existing records correctly

Keep existing wallets, policies, distributions and completed transactions. Narrate an original recording or clearly identify saved receipts/CI terminal logs as historical evidence. A JSON receipt or a printed environment variable alone is not footage of proof generation. Do not claim old completed operations are newly executing. Do not show private keys, enrollment secrets, inner proof journals or wallet storage.

The existing real-proof run is https://github.com/NotRithik/commons-for-logos/actions/runs/34309398475. It includes both threshold and allowlist-smoke jobs. The detailed recording guides explain the existing native flows and their receipt IDs. One video may have clearly separated chapters for both prizes; each PR must link its relevant complete segment. Keep the missing-video criterion unchecked until the footage is actually supplied.
