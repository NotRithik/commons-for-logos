use commons_logos_testnet_primitives::{GroupInstruction, execute_group};
use lee_core::program::{ProgramInput, ProgramOutput, read_lee_inputs};

fn main() {
    let (
        ProgramInput {
            self_program_id,
            caller_program_id,
            pre_states,
            instruction,
        },
        instruction_words,
    ) = read_lee_inputs::<GroupInstruction>();

    let post_states =
        execute_group(self_program_id, &pre_states, instruction).unwrap_or_else(|error| {
            panic!("{error}");
        });

    ProgramOutput::new(
        self_program_id,
        caller_program_id,
        instruction_words,
        pre_states,
        post_states,
    )
    .write();
}
