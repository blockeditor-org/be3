use super::*;

#[test]
fn a_number_key_picks_a_tool_and_a_click_places_a_gate() {
    let (mut editor, block) = editor();

    editor.key_press(Key::Four);
    editor.run();
    assert!(
        editor.shown("logic-grid.slot.3.0"),
        "the fourth slot is the Logic folder, which opens beside the hotbar"
    );
    editor.key_press(Key::One);
    editor.run();
    editor.click_at(canvas_point(&editor, Vec2::new(30.0, 30.0)));
    editor.run();
    editor.run();

    let grid = block.read().unwrap();
    assert_eq!(
        grid.grid().components().count(),
        1,
        "the click placed a gate"
    );
    drop(grid);
    editor.snapshot("a_number_key_picks_a_tool_and_a_click_places_a_gate");
}
