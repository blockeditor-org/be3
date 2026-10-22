use beui::{Key, Modifiers};
use block_ui_test::BeuiTest;

use super::editor;
use crate::app::TextApp;

#[test]
fn the_keyboard_reaches_the_format_bar_and_walks_it_with_the_arrows() {
    let mut editor = editor("hello");
    editor.click("text.surface");
    editor.run();
    assert!(focused(&editor, "text.surface"));

    editor.key_press(Key::Escape);
    editor.run();
    editor.key_press(Key::Tab);
    editor.run();
    assert!(
        focused(&editor, "text.format.heading"),
        "Tab out of the text lands on the bar's first button"
    );
    assert!(
        editor.shown("text.format.bold"),
        "the bar stays while the focus is in it"
    );

    editor.key_press(Key::ArrowRight);
    editor.run();
    assert!(focused(&editor, "text.format.bold"));
    editor.key_press(Key::End);
    editor.run();
    assert!(focused(&editor, "text.format.image"));
    editor.key_press(Key::ArrowRight);
    editor.run();
    assert!(
        focused(&editor, "text.format.image"),
        "the arrows stop at the ends"
    );
    editor.key_press(Key::Home);
    editor.run();
    assert!(focused(&editor, "text.format.heading"));

    editor.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    editor.run();
    assert!(focused(&editor, "text.surface"));
    assert!(editor.shown("text.format.bold"));
}

fn focused(editor: &BeuiTest<TextApp>, test_id: &str) -> bool {
    let document = editor.document();
    document
        .find_test_id(test_id)
        .is_some_and(|node| document.focus_is_within(node))
}
