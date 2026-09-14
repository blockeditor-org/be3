use super::*;

#[test]
fn a_tab_walks_back_and_forward_through_its_history() {
    let (mut editor, host, opened) = editor();
    let second = Uuid::new_v4();

    host.show_block(opened, FileTree::TYPE_ID, None, None);
    editor.step();
    editor.app().navigate_active(second, FileTree::TYPE_ID);
    editor.step();
    assert_eq!(editor.app().open_blocks(), vec![second]);
    assert_eq!(host.focused_block().block_id, Some(second));

    editor.find("workspace.back").click();
    editor.step();
    editor.step();
    assert_eq!(editor.app().open_blocks(), vec![opened]);
    assert_eq!(host.focused_block().block_id, Some(opened));

    editor.find("workspace.forward").click();
    editor.step();
    editor.step();
    assert_eq!(editor.app().open_blocks(), vec![second]);
    assert_eq!(host.focused_block().block_id, Some(second));
}
