use super::*;

use block_editor_beui::{BlockCommand, BlockHistory};

#[test]
fn undo_in_the_menu_asks_the_host_for_a_block_it_cannot_open() {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<ChildApp>::new(Editor::new(host, block)).with_top_bar(false);

    assert!(!test.menu_entry("editor.undo").enabled);
    test.pick_menu("editor.undo");
    assert!(
        test.take_block_commands().is_empty(),
        "undo asked the host with nothing to undo"
    );

    test.set_histories([(
        block,
        BlockHistory {
            can_undo: true,
            can_redo: false,
        },
    )]);
    test.run();
    assert!(test.menu_entry("editor.undo").enabled);
    test.pick_menu("editor.undo");

    assert_eq!(test.take_block_commands(), [(block, BlockCommand::Undo)]);
}
