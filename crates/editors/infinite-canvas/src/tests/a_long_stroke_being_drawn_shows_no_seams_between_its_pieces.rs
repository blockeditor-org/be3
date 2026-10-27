use super::*;
use block_editor_beui::beui::{PenPhase, PointerButton};

#[test]
fn a_long_stroke_being_drawn_shows_no_seams_between_its_pieces() {
    let mut editor = editor(&[]);
    editor.click("infinite-canvas.dock.tool.Pen");
    let from = editor.rect_of("infinite-canvas.canvas").center() - Vec2::new(250.0, 0.0);
    let at = |along: f32| {
        from + Vec2::new(
            500.0 * along,
            120.0 * (along * 9.0).sin() + 40.0 * (along * 31.0).cos(),
        )
    };
    editor.pen(PenPhase::Down(PointerButton::Primary), from, 0.4);
    let steps = 600;
    for step in 1..=steps {
        let along = step as f32 / steps as f32;
        editor.next_frame();
        editor.pen(
            PenPhase::Move,
            at(along),
            0.3 + 0.6 * (along * 13.0).sin().abs(),
        );
        editor.run();
    }
    editor.snapshot("a_long_stroke_being_drawn_shows_no_seams_between_its_pieces");
}
