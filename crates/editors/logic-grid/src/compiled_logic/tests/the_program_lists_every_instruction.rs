use super::*;

#[test]
fn the_program_lists_every_instruction() {
    let mut editor = editor();

    let instructions = editor
        .content::<CompiledLogicContent>(None)
        .root()
        .compiled
        .unwrap()
        .program()
        .instructions
        .clone();
    assert_eq!(
        instructions
            .iter()
            .map(Instruction::to_string)
            .collect::<Vec<_>>(),
        vec!["NOT m0 -> m1", "SAVE m1 -> s0"]
    );
    editor.snapshot("the_program_lists_every_instruction");
}
