use be_sdk_test::prelude::*;

use super::editor;

#[test]
fn the_format_bar_stays_when_the_text_loses_the_keyboard() {
    let mut editor = editor("word");
    assert!(
        editor.shown("text.format.bold"),
        "the bar shows before the text is clicked"
    );
    assert!(
        !editor.shown("text.format.done"),
        "a mouse opens no on-screen keyboard to put away"
    );

    editor.click("text.surface");
    assert!(editor.shown("text.format.bold"));

    editor.blur();
    assert!(
        editor.shown("text.format.bold"),
        "the bar does not hide once the text loses the keyboard"
    );
    editor.snapshot("the_format_bar_stays_when_the_text_loses_the_keyboard");
}
