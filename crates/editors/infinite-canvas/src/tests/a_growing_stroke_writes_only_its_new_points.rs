use super::*;
use block_editor_beui::beui::{PenPhase, PointerButton};

#[test]
fn a_growing_stroke_writes_only_its_new_points() {
    let mut editor = editor(&[]);
    editor.click("infinite-canvas.dock.tool.Pen");
    editor.run();
    let from = editor.rect_of("infinite-canvas.canvas").center() - Vec2::new(200.0, 0.0);
    let at = |along: f32| from + Vec2::new(400.0 * along, 150.0 * (along * 40.0).sin());
    editor.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    let steps = 400;
    let mut writes = Vec::new();
    for step in 1..=steps {
        editor.next_frame();
        editor.pen(PenPhase::Move, at(step as f32 / steps as f32), 0.5);
        editor.run();
        writes.extend(editor.store().take_written(None).iter().map(Vec::len));
    }
    editor.pen(PenPhase::Up(PointerButton::Primary), at(1.0), 0.0);
    editor.run();

    let points = entities(&editor)[0].points.len();
    assert!(points > steps / 2, "the stroke kept {points} points");
    let early = writes[2..12].iter().max().copied().unwrap_or_default();
    let late = writes[writes.len() - 10..]
        .iter()
        .max()
        .copied()
        .unwrap_or_default();
    assert!(
        late <= early + 16,
        "a write late in the stroke ({late} bytes) carries more than one early on ({early} bytes)"
    );
}
