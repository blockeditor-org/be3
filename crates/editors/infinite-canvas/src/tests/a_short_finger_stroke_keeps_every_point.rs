use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2};

#[test]
fn a_short_finger_stroke_keeps_every_point() {
    let mut editor = phone(&[]);

    editor.click("infinite-canvas.dock.tool.Pen");
    editor.run();
    let from = editor.rect_of("infinite-canvas.canvas").center();
    editor.finger(1, TouchPhase::Start, from);
    editor.finger(1, TouchPhase::Move, from + Vec2::new(3.0, 0.0));
    editor.run();
    editor.finger(1, TouchPhase::Move, from + Vec2::new(3.0, 3.0));
    editor.finger(1, TouchPhase::End, from + Vec2::new(3.0, 3.0));
    editor.run();

    let entities = entities(&editor);
    assert_eq!(entities.len(), 1, "the stroke adds one entity");
    assert_eq!(entities[0].shape, CanvasShape::Freehand);
    let drawn: Vec<CanvasPoint> = entities[0]
        .world_points()
        .iter()
        .map(|point| point.at())
        .collect();
    let start = drawn[0];
    let end = drawn[drawn.len() - 1];
    assert_eq!(
        (end.x - start.x, end.y - start.y),
        (3.0, 3.0),
        "the stroke runs from where the finger landed to where it lifted"
    );
}
