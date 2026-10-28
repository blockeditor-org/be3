use super::*;
use block_editor_beui::beui::{PenPhase, PointerButton};

#[test]
fn the_pen_tool_draws_a_stylus_stroke_as_wide_as_it_is_pressed() {
    let mut editor = editor(&[]);

    for _ in 0..4 {
        editor.click("infinite-canvas.pill.zoom-in");
    }
    editor.settle();
    editor.click("infinite-canvas.dock.tool.Pen");
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let from = canvas.center() - Vec2::new(150.0, 0.0);
    editor.pen(PenPhase::Move, from, 0.0);
    editor.next_frame();
    editor.pen(PenPhase::Down(PointerButton::Primary), from, 0.1);
    let at = |along: f32| from + Vec2::new(300.0 * along, 40.0 * (along * 6.0).sin());
    for step in 1..=10 {
        let along = step as f32 / 10.0;
        editor.next_frame();
        editor.pen(
            PenPhase::Move,
            at(along),
            0.1 + 0.9 * (along * std::f32::consts::PI).sin(),
        );
    }
    editor.next_frame();
    editor.pen(PenPhase::Up(PointerButton::Primary), at(1.0), 0.0);
    editor.run();

    let entities = entities(&editor);
    assert_eq!(entities.len(), 1, "the stroke adds one entity");
    assert_eq!(entities[0].shape, CanvasShape::Freehand);
    let (points, pressures) = (&entities[0].points, &entities[0].pressures);
    assert_eq!(points.len(), 11, "every move of the pen is a point");
    assert_eq!(
        pressures.len(),
        points.len(),
        "every point keeps its pressure"
    );
    let lightest = pressures.iter().copied().fold(f32::MAX, f32::min);
    let heaviest = pressures.iter().copied().fold(0.0, f32::max);
    assert!(
        lightest < 0.2 && heaviest > 0.9,
        "the pressure rises and falls with the pen, {pressures:?}"
    );
    editor.snapshot("the_pen_tool_draws_a_stylus_stroke_as_wide_as_it_is_pressed");
}
