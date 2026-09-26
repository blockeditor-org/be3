use super::*;

use beui::{Key, Modifiers};
use block_editor_plugin::BlockCommand;

#[test]
fn ctrl_z_undoes_the_block_through_the_top_bar() {
    let (mut test, block) = undoable_editor();

    test.key_press_modifiers(Modifiers::CTRL, Key::Z);
    test.run();

    assert_eq!(test.take_block_commands(), [(block, BlockCommand::Undo)]);
}
