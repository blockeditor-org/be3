use super::*;

#[test]
fn typing_continues_where_a_peer_deleted_the_text_around_the_caret() {
    let mut editor = editor("hello brave new world");
    caret_at(&mut editor, 9);

    peer_delete(&mut editor, 6..16);
    editor.run();
    assert_eq!(text(&editor), "hello world");

    editor.text("big ");
    editor.run();

    assert_eq!(text(&editor), "hello big world");
}
