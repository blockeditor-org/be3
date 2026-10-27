use super::*;
use block_editor_beui::beui::{Key, Modifiers, Pos2};
use std::f32::consts::PI;

fn draw_strokes(editor: &mut BeuiTest<CanvasApp>) {
    editor.click("infinite-canvas.dock.tool.Pen");
    let center = editor.rect_of("infinite-canvas.canvas").center();

    let slow: Vec<Pos2> = (0..=120)
        .map(|step| {
            let t = step as f32 / 120.0;
            center + Vec2::new(-180.0 + 360.0 * t, -150.0 + 45.0 * (t * 2.0 * PI).sin())
        })
        .collect();
    editor.drag_along(&slow);

    let fast: Vec<Pos2> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.0 * 4.0 * PI;
            center + Vec2::new(-180.0 + 26.0 * t - 40.0 * t.sin(), -10.0 - 45.0 * t.cos())
        })
        .collect();
    editor.drag_along(&fast);

    editor.click("infinite-canvas.dock.tool.Pen");
    let pressed =
        |t: f32| center + Vec2::new(-180.0 + 360.0 * t, 130.0 + 30.0 * (t * 3.0 * PI).sin());
    editor.pen(PenPhase::Move, pressed(0.0), 0.0);
    editor.next_frame();
    editor.pen(PenPhase::Down(PointerButton::Primary), pressed(0.0), 0.05);
    for step in 1..=40 {
        let t = step as f32 / 40.0;
        editor.next_frame();
        editor.pen(PenPhase::Move, pressed(t), 0.05 + 0.95 * (t * PI).sin());
    }
    editor.next_frame();
    editor.pen(PenPhase::Up(PointerButton::Primary), pressed(1.0), 0.0);
    editor.run();
    editor.key(Key::Escape, Modifiers::NONE);
}

#[test]
fn pen_strokes_are_smooth_curves_at_any_zoom() {
    let mut editor = editor(&[]);
    draw_strokes(&mut editor);
    editor.snapshot("pen_strokes_are_smooth_curves_at_any_zoom_100");

    let corner = editor.rect_of("infinite-canvas.canvas").right_bottom() - Vec2::splat(10.0);
    editor.zoom_view(corner, 4.0);
    editor.run();
    editor.settle();
    draw_strokes(&mut editor);
    editor.settle();
    editor.snapshot("pen_strokes_are_smooth_curves_at_any_zoom_400");
}
