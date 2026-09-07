use commons_logos_testnet_primitives::{DistributionInstruction, GroupInstruction};
use commons_logos_testnet_sdk::{
    allowlist_create, allowlist_create_data, serialize_instruction, threshold_create,
    threshold_execute, threshold_execute_data,
};

#[test]
fn allowlist_constructor_serializes_to_risc0_instruction_words() {
    let root = [9_u8; 32];
    let typed = allowlist_create(root, 10);
    let direct = risc0_zkvm::serde::to_vec(&DistributionInstruction::Create {
        root,
        member_count: 10,
    })
    .unwrap();

    assert_eq!(serialize_instruction(&typed).unwrap(), direct);
    assert_eq!(allowlist_create_data(root, 10).unwrap(), direct);
}

#[test]
fn threshold_constructors_serialize_to_risc0_instruction_words() {
    let root = [3_u8; 32];
    let typed = threshold_create(root, 3, 2, 7);
    let direct = risc0_zkvm::serde::to_vec(&GroupInstruction::Create {
        root,
        member_count: 3,
        threshold: 2,
        initial_value: 7,
    })
    .unwrap();

    assert_eq!(serialize_instruction(&typed).unwrap(), direct);
    assert_eq!(
        threshold_execute_data().unwrap(),
        risc0_zkvm::serde::to_vec(&threshold_execute()).unwrap()
    );
}
