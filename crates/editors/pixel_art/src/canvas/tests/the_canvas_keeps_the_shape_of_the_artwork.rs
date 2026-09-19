use super::*;

#[test]
fn the_canvas_keeps_the_shape_of_the_artwork() {
    let view = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0));

    let canvas = canvas_rect(view, 4, 4);

    assert_eq!(canvas.size(), Vec2::new(100.0, 100.0));
    assert_eq!(canvas.center(), view.center());
}
