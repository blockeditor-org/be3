use super::*;

use crate::drawing::Drawing;

#[test]
fn a_drawing_paints_between_the_shapes_around_it() {
    let drawing = Drawing::new(Green::default());
    let scene = Rect::from_min_max(pos2(8.0, 8.0), pos2(56.0, 56.0));
    let over = Rect::from_min_max(pos2(40.0, 40.0), pos2(56.0, 56.0));
    let capture = capture(Color32::TRANSPARENT, |painter| {
        painter.rect_filled(everything(), 0.0, Color32::WHITE);
        painter.drawing(scene, &drawing);
        painter.rect_filled(over, 0.0, Color32::from_rgb(255, 0, 0));
    });

    assert_eq!(
        capture.pixel(24, 24),
        [0, 255, 0, 255],
        "the drawing should have painted its own rectangle"
    );
    assert_eq!(
        capture.pixel(4, 4),
        [255, 255, 255, 255],
        "the fill beneath the drawing should have been left alone outside it"
    );
    assert_eq!(
        capture.pixel(48, 48),
        [255, 0, 0, 255],
        "the shape painted after the drawing should still have reached the pass"
    );
}
