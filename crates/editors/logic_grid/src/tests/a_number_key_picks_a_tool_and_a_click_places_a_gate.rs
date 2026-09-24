use super::*;

#[test]
fn a_number_key_picks_a_tool_and_a_click_places_a_gate() {
    let mut editor = editor();

    editor.key_press(Key::Four);
    editor.run();
    assert!(
        editor.shown("logic-grid.slot.3.0"),
        "the fourth slot is the Logic folder, which opens beside the hotbar"
    );
    editor.key_press(Key::One);
    editor.run();
    let point = canvas_point(&editor, Vec2::new(30.0, 30.0));
    editor.click_at(point);
    editor.run();
    editor.run();

    let grid = grid(&editor);
    assert_eq!(grid.components().count(), 1, "the click placed a gate");
    editor.snapshot("a_number_key_picks_a_tool_and_a_click_places_a_gate");
}
