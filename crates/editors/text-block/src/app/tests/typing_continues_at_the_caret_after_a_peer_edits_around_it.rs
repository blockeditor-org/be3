use super::*;
use block_editor_beui::be_block::TextOp;

#[test]
fn typing_continues_at_the_caret_after_a_peer_edits_around_it() {
    let mut editor = editor("hello world");
    caret_at(&mut editor, 6);

    editor.edit::<TextContent>(None, &TextOp::insert(0, ">> "));
    editor.edit::<TextContent>(None, &TextOp::insert(14, "!"));
    editor.run();
    assert_eq!(text(&editor), ">> hello world!");

    editor.text("big ");
    editor.run();

    assert_eq!(text(&editor), ">> hello big world!");
}
