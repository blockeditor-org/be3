use super::{editor, text};

#[test]
fn typing_inserts_text_into_the_document() {
    let (mut editor, block) = editor("one");

    editor.click("text.surface");
    editor.run();
    editor.text(" two");
    editor.run();

    assert_eq!(text(&block), "one two");
}
