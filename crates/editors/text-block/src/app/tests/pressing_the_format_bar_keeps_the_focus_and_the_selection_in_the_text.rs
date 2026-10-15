use beui::{Key, Modifiers, pos2};

use super::{editor, text};

#[test]
fn pressing_the_format_bar_keeps_the_focus_and_the_selection_in_the_text() {
    let mut editor = editor("hello world");
    editor.click("text.surface");
    editor.run();
    editor.key_press(Key::End);
    editor.run();
    for _ in 0.."world".len() {
        editor.key_press_modifiers(Modifiers::SHIFT, Key::ArrowLeft);
        editor.run();
    }

    let bar = editor.rect_of("text.format");
    editor.click_at(pos2(bar.max.x - 4.0, bar.center().y));
    editor.run();
    editor.text("there");
    editor.run();
    assert_eq!(
        text(&editor),
        "hello there",
        "a press on the bar leaves the text focused with its selection"
    );

    editor.key_press_modifiers(Modifiers::SHIFT, Key::Home);
    editor.run();
    editor.click("text.format.bold");
    editor.run();
    assert_eq!(text(&editor), "**hello there**");
    assert!(
        editor.shown("text.format.bold"),
        "the bar stays, since the text kept the focus"
    );
    editor.text("!");
    editor.run();
    assert!(
        text(&editor).contains('!'),
        "typing goes on into the text after a format button: {:?}",
        text(&editor)
    );
}
