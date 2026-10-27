use super::*;
use block_editor_beui::beui::Modifiers;

#[test]
fn a_number_key_picks_a_tool_and_a_click_places_a_gate() {
    let mut editor = editor();

    editor.key(Key::Four, Modifiers::NONE);
    assert!(
        editor.shown("logic-grid.slot.3.0"),
        "the fourth slot is the Logic folder, which opens beside the hotbar"
    );
    editor.key(Key::One, Modifiers::NONE);
    let point = canvas_point(&editor, Vec2::new(30.0, 30.0));
    editor.click(point);

    let grid = grid(&editor);
    assert_eq!(grid.components().count(), 1, "the click placed a gate");
    editor.snapshot("a_number_key_picks_a_tool_and_a_click_places_a_gate");
}
