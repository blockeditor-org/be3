use super::*;
use block_editor_beui::beui::{Rect, pos2};
use infinite_canvas_types::CanvasColor;

#[test]
fn panning_paints_no_selection_handle_again() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform = CanvasTransform::new(
        CanvasPoint::new(-60.0, 0.0),
        CanvasPoint::new(120.0, 80.0),
        0.0,
    );
    rectangle.style.fill = CanvasFill::Solid;
    rectangle.style.fill_color = CanvasColor::Rgba {
        red: 60,
        green: 110,
        blue: 90,
        alpha: 255,
    };
    let mut canvas = vector([rectangle.clone()]);
    canvas.grid.hidden = true;
    let mut editor = open(&canvas, false, &[]);
    let view = Rect::from_min_size(pos2(0.0, 0.0), editor.rect().size());
    editor.set_view(view, 1.0);
    editor.run();
    editor.click(format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();

    editor.set_view(view.translate(Vec2::new(40.0, 25.0)), 1.0);
    editor.step(Vec::new());

    assert_eq!(
        editor.document().performance().latest.work.painted_nodes,
        1,
        "a pan paints the selection's outline or handles again, not only the canvas"
    );
}
