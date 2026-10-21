use super::*;
use block_editor_beui::beui::{Rect, pos2};

fn painted_by_a_pan(frames: u128) -> usize {
    let mut canvas = vector(
        (0..frames)
            .map(|index| {
                frame(
                    index + 1,
                    CanvasPoint::new(index as f32 * 140.0 - 200.0, 0.0),
                    CanvasPoint::new(120.0, 90.0),
                )
            })
            .collect::<Vec<_>>(),
    );
    canvas.grid.hidden = true;
    let mut editor = open(&canvas, false, &[]);
    let view = Rect::from_min_size(pos2(0.0, 0.0), editor.rect().size());
    editor.set_view(view, 1.0);
    editor.run();
    editor.set_view(view.translate(Vec2::new(40.0, 25.0)), 1.0);
    editor.step(Vec::new());
    editor.document().performance().latest.work.painted_nodes
}

#[test]
fn panning_paints_no_frame_backdrop_again() {
    assert_eq!(
        painted_by_a_pan(1),
        painted_by_a_pan(4),
        "a pan paints the backdrop, outline or label of a frame again"
    );
}
