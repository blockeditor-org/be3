use super::*;

#[test]
fn quick_typing_undoes_a_word_at_a_time() {
    let mut editor = editor("one");
    editor.store().keep_history::<TextContent>(None);
    editor.run();
    caret_at(&mut editor, 3);

    editor.type_text(" two three");
    editor.run();
    assert_eq!(text(&editor), "one two three");

    let block = editor.block_id().expect("the editor has a block");
    editor.host().undo(block);
    editor.run();
    assert_eq!(text(&editor), "one two", "the last word goes first");
    editor.host().undo(block);
    editor.run();
    assert_eq!(text(&editor), "one");
}
