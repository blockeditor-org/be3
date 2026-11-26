use super::*;
use be_sdk_beui::beui::{PenPhase, PointerButton};

/// The window asks for every sample from the frame after it asks, so the pen tool asks while
/// it waits for a press: the moves in the frame of a stroke's press, under the drag threshold
/// too, all reach the stroke.
#[test]
fn the_pen_tool_asks_for_every_sample_before_its_stroke_begins() {
    let mut editor = editor(&[]);
    assert!(
        !editor.window().every_sample,
        "the select tool lets the host merge a frame's moves"
    );

    editor.click("infinite-canvas.dock.tool.Pen");
    assert!(
        editor.window().every_sample,
        "the pen tool asks for every sample before it is pressed"
    );

    let from = editor.rect_of("infinite-canvas.canvas").center();
    editor.pen(PenPhase::Move, from, 0.0);
    editor.in_one_frame(|editor| {
        editor.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
        for step in 1..=3 {
            editor.pen(PenPhase::Move, from + Vec2::new(1.2 * step as f32, 0.0), 0.5);
        }
    });
    editor.pen(PenPhase::Up(PointerButton::Primary), from + Vec2::new(3.6, 0.0), 0.0);

    let entities = entities(&editor);
    assert_eq!(entities[0].shape, CanvasShape::Freehand);
    assert_eq!(
        entities[0].points.len(),
        4,
        "the press and each move short of the threshold in its frame are points"
    );
}
