use super::*;

use beui::{Key, Modifiers};
use block_editor_beui::BlockCommand;

#[test]
fn ctrl_z_undoes_a_block_that_owns_its_frame_without_a_top_bar() {
    let block = Uuid::new_v4();
    let mut test = BeuiTest::<ChildApp>::editing(block).without_top_bar();
    test.set_histories([(
        block,
        block_editor_beui::BlockHistory {
            head: None,
            can_undo: true,
            can_redo: false,
        },
    )]);
    test.run();

    test.key(Key::Z, Modifiers::CTRL);
    test.run();

    assert_eq!(test.take_block_commands(), [(block, BlockCommand::Undo)]);
}
