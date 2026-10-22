use super::*;
use block_editor_beui::beui::{Rect, pos2};

fn painted_by_a_pan(hidden: bool) -> usize {
    let mut canvas = Canvas::with_entities(vec![card()]);
    canvas.grid.hidden = hidden;
    let mut editor = open(&canvas, false, &[]);
    let view = Rect::from_min_size(pos2(0.0, 0.0), editor.rect().size());
    editor.set_view(view, 1.0);
    editor.run();
    editor.set_view(view.translate(Vec2::new(40.0, 25.0)), 1.0);
    editor.step(Vec::new());
    editor.document().performance().latest.work.painted_nodes
}

#[test]
fn panning_paints_no_grid_dot_again() {
    assert_eq!(
        painted_by_a_pan(false),
        painted_by_a_pan(true),
        "a pan painted more nodes with the grid shown than without it"
    );
}
