// GENERATED IDL declaration, NOT executable guest code.
#[lez_program(instruction = "astra_logos_testnet_primitives::GroupInstruction")]
mod astra_threshold_v024 {
 #[instruction]
 pub fn create(
 #[account(mut, init, signer)] state: AccountWithMetadata,
 root: Hash32,
 member_count: u32,
 threshold: u32,
 initial_value: i64,
) {}
 #[instruction]
 pub fn propose(
 #[account(mut)] state: AccountWithMetadata,
 #[account(signer)] member: AccountWithMetadata,
 witness: MemberWitness,
 next_value: i64,
) {}
 #[instruction]
 pub fn approve(
 #[account(mut)] state: AccountWithMetadata,
 #[account(signer)] member: AccountWithMetadata,
 witness: MemberWitness,
) {}
 #[instruction]
 pub fn execute(
 #[account(mut)] state: AccountWithMetadata,
) {}
}
