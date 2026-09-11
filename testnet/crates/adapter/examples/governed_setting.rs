//! A real LEZ guest consuming another program's authenticated public policy state.
use commons_logos_policy_adapter::{GovernedSettingInstruction, execute_governed_setting};
use lee_core::program::{ProgramInput, ProgramOutput, read_lee_inputs};

fn main() {
    let (
        ProgramInput {
            self_program_id,
            caller_program_id,
            pre_states,
            instruction,
        },
        words,
    ) = read_lee_inputs::<GovernedSettingInstruction>();
    let post_states = execute_governed_setting(self_program_id, &pre_states, instruction)
        .unwrap_or_else(|error| panic!("{error}"));
    ProgramOutput::new(
        self_program_id,
        caller_program_id,
        words,
        pre_states,
        post_states,
    )
    .write();
}
