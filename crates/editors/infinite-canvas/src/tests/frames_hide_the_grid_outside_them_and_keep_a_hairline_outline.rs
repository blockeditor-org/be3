use super::*;
use block_editor_beui::beui::{Rect, pos2};

fn framed(hidden: bool) -> BeuiTest<CanvasApp> {
    let mut canvas = vector([frame(
        2,
        CanvasPoint::new(0.0, 0.0),
        CanvasPoint::new(240.0, 160.0),
    )]);
    canvas.grid.hidden = hidden;
    open(&canvas, false, &[])
}

fn painted_by_a_pan(hidden: bool) -> usize {
    let mut editor = framed(hidden);
    let view = Rect::from_min_size(pos2(0.0, 0.0), editor.rect().size());
    editor.set_view(view, 1.0);
    editor.run();
    editor.set_view(view.translate(Vec2::new(40.0, 25.0)), 1.0);
    editor.step(Vec::new());
    editor.document().performance().latest.work.painted_nodes
}

#[test]
fn frames_hide_the_grid_outside_them_and_keep_a_hairline_outline() {
    let mut editor = framed(false);
    let view = Rect::from_min_size(pos2(0.0, 0.0), editor.rect().size());
    for scale in [1.0, 2.0] {
        editor.set_view(view, scale);
        editor.settle();
        editor.snapshot(&format!(
            "frames_hide_the_grid_outside_them_and_keep_a_hairline_outline_at_{scale}x"
        ));
    }

    assert_eq!(
        painted_by_a_pan(false),
        painted_by_a_pan(true),
        "a pan paints no grid dot again when frames clip the grid"
    );
}
