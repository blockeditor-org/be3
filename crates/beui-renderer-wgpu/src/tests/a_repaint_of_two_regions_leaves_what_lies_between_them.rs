use super::*;

#[test]
fn a_repaint_of_two_regions_leaves_what_lies_between_them() {
    let left = Rect::from_min_max(pos2(4.0, 4.0), pos2(20.0, 20.0));
    let middle = Rect::from_min_max(pos2(24.0, 24.0), pos2(40.0, 40.0));
    let right = Rect::from_min_max(pos2(44.0, 44.0), pos2(60.0, 60.0));
    let mut target = Target::new();
    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        for square in [left, middle, right] {
            painter.rect_filled(square, 0.0, Color32::WHITE);
        }
    });

    target.draw(
        Color32::BLACK,
        Repaint::Region {
            region: Region::from(left).union(right.into()),
            background: Color32::BLACK,
        },
        |_| {},
    );
    let capture = target.read();

    assert_eq!(
        capture.pixel(12, 12),
        [0, 0, 0, 255],
        "the first region is repainted"
    );
    assert_eq!(
        capture.pixel(52, 52),
        [0, 0, 0, 255],
        "the second region is repainted"
    );
    assert_eq!(
        capture.pixel(32, 32),
        [255, 255, 255, 255],
        "what lies between the two regions keeps the retained frame"
    );
}
