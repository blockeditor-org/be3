use super::*;
use block_editor_beui::beui::TouchPhase;

#[test]
fn a_finger_on_the_pen_tool_draws_before_it_moves() {
    let mut editor = phone(&[]);

    editor.click("infinite-canvas.dock.tool.Pen");
    let at = editor.rect_of("infinite-canvas.canvas").center();
    editor.fingers(TouchPhase::Start, &[(1, at)]);
    editor.snapshot("a_finger_on_the_pen_tool_draws_before_it_moves");
    editor.fingers(TouchPhase::End, &[(1, at)]);

    let entities = entities(&editor);
    assert_eq!(entities.len(), 1, "the tap adds one stroke");
    assert_eq!(entities[0].shape, CanvasShape::Freehand);
    assert_eq!(
        (entities[0].points.len(), entities[0].pressures.len()),
        (1, 1),
        "a tap is kept as a dot"
    );
}
