use super::*;

use block_editor_plugin::{BlockCommand, BlockHistory};

#[test]
fn undo_in_the_top_bar_asks_the_host_for_a_block_it_cannot_open() {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<ChildApp>::new(Editor::new(host.clone(), block)).with_top_bar(false);

    test.click("editor.undo");
    test.run();
    assert!(
        host.take_block_commands().is_empty(),
        "undo asked the host with nothing to undo"
    );

    host.set_histories([(
        block,
        BlockHistory {
            can_undo: true,
            can_redo: false,
        },
    )]);
    test.run();
    test.click("editor.undo");
    test.run();

    assert_eq!(host.take_block_commands(), [(block, BlockCommand::Undo)]);
}
