use super::*;

#[test]
fn a_block_opened_from_a_tab_replaces_it() {
    let (mut editor, host, opened) = editor();
    let linked = Uuid::new_v4();

    host.show_block(opened, FileTree::TYPE_ID, None, None);
    editor.step();
    host.show_block(linked, FileTree::TYPE_ID, None, Some(opened));
    editor.step();

    assert_eq!(editor.app().open_blocks(), vec![linked]);

    editor.find("workspace.back").click();
    editor.step();
    editor.step();
    assert_eq!(editor.app().open_blocks(), vec![opened]);
}
