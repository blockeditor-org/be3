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

    editor.key(Key::Escape, Modifiers::NONE);
    editor.run();
    editor.key(Key::Tab, Modifiers::NONE);
    editor.run();
    assert!(
        focused(&editor, "text.format.heading"),
        "Tab out of the text lands on the bar's first button"
    );
    assert!(
        editor.shown("text.format.bold"),
        "the bar stays while the focus is in it"
    );

    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.run();
    assert!(focused(&editor, "text.format.bold"));
    editor.key(Key::End, Modifiers::NONE);
    editor.run();
    assert!(focused(&editor, "text.format.image"));
    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.run();
    assert!(
        focused(&editor, "text.format.image"),
        "the arrows stop at the ends"
    );
    editor.key(Key::Home, Modifiers::NONE);
    editor.run();
    assert!(focused(&editor, "text.format.heading"));

    editor.key(Key::Tab, Modifiers::SHIFT);
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
