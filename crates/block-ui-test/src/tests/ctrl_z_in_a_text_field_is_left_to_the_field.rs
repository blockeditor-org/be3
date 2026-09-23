use super::*;

use beui::{Key, Modifiers};

#[test]
fn ctrl_z_in_a_text_field_is_left_to_the_field() {
    let (mut test, host, _) = undoable_editor();
    test.click("editor.name");
    test.run();

    test.key_press_modifiers(Modifiers::CTRL, Key::Z);
    test.run();

    assert!(
        host.take_block_commands().is_empty(),
        "undo in a focused text field undoes the typing, not the block"
    );
}
