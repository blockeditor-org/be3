use super::*;

#[test]
fn a_peers_edit_in_the_middle_of_a_word_leaves_it_one_undo_step() {
    let mut editor = editor("one");
    editor.store().keep_history::<TextContent>(None);
    editor.run();
    caret_at(&mut editor, 3);

    editor.type_text(" tw");
    editor.run();
    peer_insert(&mut editor, 0, ">> ");
    editor.run();
    editor.type_text("o");
    editor.run();
    assert_eq!(text(&editor), ">> one two");

    let block = editor.block_id().expect("the editor has a block");
    editor.host().undo(block);
    editor.run();
    assert_eq!(
        text(&editor),
        ">> one",
        "the peer's edit does not cut the word typed around it"
    );
}
