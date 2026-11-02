use super::*;

use be_sdk_beui::BlockCommand;
use beui::{Key, Modifiers};

#[test]
fn ctrl_z_undoes_the_block_through_the_frames_undo() {
    let (mut test, block) = undoable_editor();

    test.key(Key::Z, Modifiers::CTRL);
    test.run();

    assert_eq!(test.take_block_commands(), [(block, BlockCommand::Undo)]);
}
