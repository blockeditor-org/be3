use be_sdk_test::prelude::*;
use super::{editor, text};

#[test]
fn typing_inserts_text_into_the_document() {
    let mut editor = editor("one");

    editor.click("text.surface");
    editor.type_text(" two");

    assert_eq!(text(&editor), "one two");
}
