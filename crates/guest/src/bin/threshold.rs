use astra_logos_primitives::{GroupInstruction, execute_group};
use lee_core::program::{ProgramCall, ProgramOutput, read_lee_call, respond_unsupported_call};

fn main() {
    let call = read_lee_call::<GroupInstruction>();
    let ProgramCall::Execute(input, instruction_data) = call else {
        respond_unsupported_call(call);
    };
    let diffs = execute_group(input.self_account_id, &input.pre_states, input.instruction)
        .unwrap_or_else(|error| panic!("{error}"));
    ProgramOutput::new(
        input.self_account_id,
        input.caller_account_id,
        instruction_data,
        diffs,
    )
    .write();
}
