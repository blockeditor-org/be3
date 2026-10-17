use super::*;

#[test]
fn selecting_a_grapheme_keeps_prepend_with_ascii() {
    let mut editor = editor("x\u{600}ay");
    caret_at(&mut editor, 1);
    editor.key_press_modifiers(beui::Modifiers::SHIFT, Key::ArrowRight);
    editor.run();
    editor.snapshot("selecting_a_grapheme_keeps_prepend_with_ascii");
    editor.key_press(Key::Backspace);
    editor.run();
    assert_eq!(text(&editor), "xy");
}
