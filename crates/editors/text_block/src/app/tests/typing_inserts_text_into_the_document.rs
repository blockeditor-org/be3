use super::{editor, text};

#[test]
fn typing_inserts_text_into_the_document() {
    let mut editor = editor("one");

    editor.click("text.surface");
    editor.run();
    editor.text(" two");
    editor.run();
    editor.run();

    assert_eq!(text(&editor), "one two");
}
