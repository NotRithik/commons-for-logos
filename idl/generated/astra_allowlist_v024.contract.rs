// GENERATED IDL declaration, NOT executable guest code.
#[lez_program(instruction = "astra_logos_testnet_primitives::DistributionInstruction")]
mod astra_allowlist_v024 {
 #[instruction]
 pub fn create(
 #[account(mut, init, signer)] state: AccountWithMetadata,
 root: Hash32,
 member_count: u32,
) {}
 #[instruction]
 pub fn claim(
 #[account(mut)] state: AccountWithMetadata,
 #[account(signer)] member: AccountWithMetadata,
 witness: MemberWitness,
) {}
}
