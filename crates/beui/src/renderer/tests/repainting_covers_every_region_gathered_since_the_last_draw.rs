use super::*;

#[test]
fn repainting_covers_every_region_gathered_since_the_last_draw() {
    let left = Rect::from_min_max(pos2(4.0, 4.0), pos2(28.0, 28.0));
    let right = Rect::from_min_max(pos2(36.0, 36.0), pos2(60.0, 60.0));
    let mut target = Target::new();
    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(left, 0.0, Color32::WHITE);
        painter.rect_filled(right, 0.0, Color32::WHITE);
    });

    let region = Repaint::Region {
        region: left,
        background: Color32::BLACK,
    }
    .union(Repaint::Region {
        region: right,
        background: Color32::BLACK,
    });
    target.draw(Color32::BLACK, region, |_| {});
    let capture = target.read();

    assert_eq!(
        capture.pixel(16, 16),
        [0, 0, 0, 255],
        "a region gathered before the draw is still repainted"
    );
    assert_eq!(
        capture.pixel(48, 48),
        [0, 0, 0, 255],
        "the region gathered last is repainted too"
    );
}
