use beui::{Key, Modifiers, pos2};
use block_ui_test::prelude::*;

use super::{editor, text};

#[test]
fn pressing_the_format_bar_keeps_the_focus_and_the_selection_in_the_text() {
    let mut editor = editor("hello world");
    editor.click("text.surface");
    editor.key(Key::End, Modifiers::NONE);
    for _ in 0.."world".len() {
        editor.key(Key::ArrowLeft, Modifiers::SHIFT);
    }

    let bar = editor.rect_of("text.format");
    editor.click(pos2(bar.max.x - 4.0, bar.center().y));
    editor.type_text("there");
    assert_eq!(
        text(&editor),
        "hello there",
        "a press on the bar leaves the text focused with its selection"
    );

    editor.key(Key::Home, Modifiers::SHIFT);
    editor.click("text.format.bold");
    assert_eq!(text(&editor), "**hello there**");
    assert!(
        editor.shown("text.format.bold"),
        "the bar stays, since the text kept the focus"
    );
    editor.type_text("!");
    assert!(
        text(&editor).contains('!'),
        "typing goes on into the text after a format button: {:?}",
        text(&editor)
    );
}
