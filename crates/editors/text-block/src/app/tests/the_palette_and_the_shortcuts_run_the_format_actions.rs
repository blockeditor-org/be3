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
    editor.key(Key::B, ctrl);
    editor.run();
    assert!(text(&editor).contains("**"), "{:?}", text(&editor));

    editor.key(
        Key::P,
        Modifiers {
            shift: true,
            ..ctrl
        },
    );
    editor.run();
    editor.type_text("heading");
    editor.run();
    editor.key(Key::Enter, Modifiers::NONE);
    editor.run();
    assert!(text(&editor).starts_with("## "), "{:?}", text(&editor));

    editor.type_text("!");
    editor.run();
    assert!(
        text(&editor).contains('!'),
        "the text has the focus again once the palette closes: {:?}",
        text(&editor)
    );
}
