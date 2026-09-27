use super::*;

use crate::drawing::Drawing;

#[test]
fn a_drawing_repaints_only_within_each_damaged_region() {
    let left = Rect::from_min_max(pos2(4.0, 4.0), pos2(20.0, 20.0));
    let right = Rect::from_min_max(pos2(44.0, 44.0), pos2(60.0, 60.0));
    let over = Rect::from_min_max(pos2(4.0, 4.0), pos2(12.0, 12.0));
    let drawing = Drawing::new(Green::default());
    let mut target = Target::new();
    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(everything(), 0.0, Color32::WHITE);
    });

    target.draw(
        Color32::BLACK,
        Repaint::Region {
            region: Region::from(left).union(right.into()),
            background: Color32::BLACK,
        },
        |painter| {
            painter.drawing(everything(), &drawing);
            painter.rect_filled(over, 0.0, Color32::from_rgb(255, 0, 0));
        },
    );
    let capture = target.read();

    assert_eq!(
        capture.pixel(16, 16),
        [0, 255, 0, 255],
        "the drawing is repainted in the first region"
    );
    assert_eq!(
        capture.pixel(52, 52),
        [0, 255, 0, 255],
        "the drawing is repainted in the second region"
    );
    assert_eq!(
        capture.pixel(32, 32),
        [255, 255, 255, 255],
        "what lies between the two regions keeps the retained frame"
    );
    assert_eq!(
        capture.pixel(8, 8),
        [255, 0, 0, 255],
        "a shape painted after the drawing is still held to its region"
    );
    assert_eq!(
        capture.pixel(2, 2),
        [255, 255, 255, 255],
        "nothing is painted outside the damaged regions"
    );
}
