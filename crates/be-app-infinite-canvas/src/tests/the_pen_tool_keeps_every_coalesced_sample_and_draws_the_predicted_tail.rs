use super::*;
use be_sdk_beui::beui::{PenPhase, PointerButton};

#[test]
fn the_pen_tool_keeps_every_coalesced_sample_and_draws_the_predicted_tail() {
    let mut editor = editor(&[]);

    editor.click("infinite-canvas.dock.tool.Pen");
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let from = canvas.center() - Vec2::new(150.0, 0.0);
    let at = |along: f32| from + Vec2::new(200.0 * along, 60.0 * (along * 3.0).sin());
    editor.pen(PenPhase::Move, from, 0.0);
    editor.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    editor.in_one_frame(|editor| {
        for step in 1..=8 {
            editor.pen(PenPhase::Move, at(step as f32 / 10.0), 0.5);
        }
        editor.predict(at(1.5));
    });
    editor.snapshot("the_pen_tool_keeps_every_coalesced_sample_and_draws_the_predicted_tail");

    editor.pen(PenPhase::Move, at(0.9), 0.5);
    editor.snapshot("the_pen_tool_replaces_the_predicted_tail_with_the_next_sample");
    editor.pen(PenPhase::Up(PointerButton::Primary), at(0.9), 0.0);

    let entities = entities(&editor);
    assert_eq!(entities[0].shape, CanvasShape::Freehand);
    assert_eq!(
        entities[0].points.len(),
        10,
        "every sample delivered in one frame is a point, and the prediction is not kept"
    );
}
