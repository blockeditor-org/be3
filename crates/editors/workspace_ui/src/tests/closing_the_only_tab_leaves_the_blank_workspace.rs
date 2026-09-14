use super::*;

#[test]
fn closing_the_only_tab_leaves_the_blank_workspace() {
    let (mut editor, host, opened) = editor();

    host.show_block(opened, FileTree::TYPE_ID, None, None);
    editor.step();
    assert_eq!(editor.app().open_blocks(), vec![opened]);

    editor.app().close_active();
    editor.step();

    assert!(editor.app().open_blocks().is_empty());
    assert_eq!(host.focused_block().block_id, None);
}
