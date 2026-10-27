use super::{editor, text};
use block_ui_test::prelude::*;

#[test]
fn typing_inserts_text_into_the_document() {
    let mut editor = editor("one");

    editor.click("text.surface");
    editor.type_text(" two");

    assert_eq!(text(&editor), "one two");
}
