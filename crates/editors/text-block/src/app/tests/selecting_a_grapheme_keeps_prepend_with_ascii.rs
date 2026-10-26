use super::*;
use block_editor_beui::beui::Modifiers;

#[test]
fn selecting_a_grapheme_keeps_prepend_with_ascii() {
    let mut editor = editor("x\u{600}ay");
    caret_at(&mut editor, 1);
    editor.key(Key::ArrowRight, beui::Modifiers::SHIFT);
    editor.run();
    editor.snapshot("selecting_a_grapheme_keeps_prepend_with_ascii");
    editor.key(Key::Backspace, Modifiers::NONE);
    editor.run();
    assert_eq!(text(&editor), "xy");
}
