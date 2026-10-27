use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn the_ellipse_and_diamond_tools_draw_their_shapes() {
    let mut editor = editor(&[]);
    let canvas = editor.rect_of("infinite-canvas.canvas");

    for (tool, offset) in [("Ellipse", -150.0), ("Diamond", 30.0)] {
        editor.click(format!("infinite-canvas.dock.tool.{tool}"));
        let from = canvas.center() + Vec2::new(offset, -40.0);
        editor.drag(from, from + Vec2::new(120.0, 80.0));
    }

    let shapes: Vec<_> = entities(&editor)
        .iter()
        .map(|entity| (entity.shape, entity.transform.size))
        .collect();
    assert_eq!(
        shapes,
        [
            (CanvasShape::Ellipse, CanvasPoint::new(120.0, 80.0)),
            (CanvasShape::Diamond, CanvasPoint::new(120.0, 80.0)),
        ]
    );
}
