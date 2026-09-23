use super::*;

#[test]
fn instructions_have_compact_display_names() {
    assert_eq!(
        Instruction::Not {
            input: 2,
            output: 5,
        }
        .to_string(),
        "NOT m2 -> m5"
    );
    assert_eq!(
        Instruction::ReadStorage {
            storage: 3,
            output: 4,
        }
        .to_string(),
        "READ s3 -> m4"
    );
    assert_eq!(
        Instruction::SaveStorage {
            storage: 3,
            input: 4,
        }
        .to_string(),
        "SAVE m4 -> s3"
    );
}
