use super::*;

#[test]
fn a_shown_block_is_reported_as_focused() {
    let (mut editor, host, opened) = editor();
    let second = Uuid::new_v4();

    host.show_block(opened, FileTree::TYPE_ID, None, None);
    editor.step();
    assert_eq!(host.focused_block().block_id, Some(opened));
    assert_eq!(host.focused_block().block_type, FileTree::TYPE_ID);

    host.show_block(second, FileTree::TYPE_ID, Some(opened), None);
    editor.step();
    assert_eq!(editor.app().open_blocks(), vec![opened, second]);
    assert_eq!(host.focused_block().block_id, Some(second));
    assert_eq!(host.focused_block().via, vec![opened]);
}
