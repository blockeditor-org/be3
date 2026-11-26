use super::*;

#[test]
fn typing_follows_a_replacement_of_the_carets_byte() {
    let mut editor = editor("hello world");
    caret_at(&mut editor, 5);

    peer_delete(&mut editor, 5..6);
    peer_insert(&mut editor, 5, " big ");
    editor.run();
    assert_eq!(text(&editor), "hello big world");
    editor.record();

    editor.type_text("wide ");
    assert_eq!(text(&editor), "hello big wide world");
    editor.record();
    editor.snapshot("typing_follows_a_replacement_of_the_carets_byte");
}
