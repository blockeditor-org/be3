use beui::{Key, Modifiers};

use super::{editor, text};

#[test]
fn the_palette_and_the_shortcuts_run_the_format_actions() {
    let mut editor = editor("word");
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };

    editor.click("text.surface");
    editor.run();
    editor.key_press_modifiers(ctrl, Key::B);
    editor.run();
    assert!(text(&editor).contains("**"), "{:?}", text(&editor));

    editor.key_press_modifiers(
        Modifiers {
            shift: true,
            ..ctrl
        },
        Key::P,
    );
    editor.run();
    editor.text("heading");
    editor.run();
    editor.key_press(Key::Enter);
    editor.run();
    assert!(text(&editor).starts_with("## "), "{:?}", text(&editor));

    editor.text("!");
    editor.run();
    assert!(
        text(&editor).contains('!'),
        "the text has the focus again once the palette closes: {:?}",
        text(&editor)
    );
}
