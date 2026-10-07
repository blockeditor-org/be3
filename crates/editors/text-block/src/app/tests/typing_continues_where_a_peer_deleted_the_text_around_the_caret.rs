use super::*;
use block_editor_beui::be_block::TextOp;

#[test]
fn typing_continues_where_a_peer_deleted_the_text_around_the_caret() {
    let mut editor = editor("hello brave new world");
    caret_at(&mut editor, 9);

    editor.edit::<TextContent>(None, &TextOp::delete(6, 10));
    editor.run();
    assert_eq!(text(&editor), "hello world");

    editor.text("big ");
    editor.run();

    assert_eq!(text(&editor), "hello big world");
}
