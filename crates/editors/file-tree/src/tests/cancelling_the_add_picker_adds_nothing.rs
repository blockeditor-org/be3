use block_editor_beui::{BlockCommand, ChildContent};

use super::*;

#[test]
fn cancelling_the_add_picker_adds_nothing() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let files = my_files(&mut fixture, Vec::new());
    fixture.settle();

    fixture.test.click(format!("file-tree.{files}.add"));
    fixture.settle();
    let picker = fixture
        .test
        .placed_picker()
        .expect("the add button opens a picker");
    assert!(matches!(picker.content, ChildContent::Picker { .. }));

    fixture.test.answer_picker(Ok(None));
    fixture.settle();
    assert!(
        fixture.test.placed_picker().is_none(),
        "a cancelled picker closes"
    );
    assert!(
        !fixture
            .test
            .take_block_commands()
            .iter()
            .any(|(_, command)| matches!(command, BlockCommand::Place { .. })),
        "and nothing is placed"
    );
}
