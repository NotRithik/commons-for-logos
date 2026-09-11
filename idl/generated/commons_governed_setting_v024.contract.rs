// GENERATED IDL declaration, NOT executable guest code.
#[lez_program(instruction = "commons_logos_policy_adapter::GovernedSettingInstruction")]
mod commons_governed_setting_v024 {
 #[instruction]
 pub fn initialize(
 #[account(mut, init, signer)] consumer: AccountWithMetadata,
 #[account()] policy_state: AccountWithMetadata,
 policy: PolicyBinding,
 minimum: i64,
 maximum: i64,
 initial_value: i64,
) {}
 #[instruction]
 pub fn apply(
 #[account(mut)] consumer: AccountWithMetadata,
 #[account()] policy_state: AccountWithMetadata,
 expected_sequence: u64,
 expected_value: i64,
) {}
}
