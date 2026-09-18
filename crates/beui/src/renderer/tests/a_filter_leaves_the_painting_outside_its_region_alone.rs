use super::*;

#[test]
fn a_filter_leaves_the_painting_outside_its_region_alone() {
    let capture = capture(Color32::BLACK, |painter| {
        painter.rect_filled(everything(), 0.0, Color32::WHITE);
        painter.ctx().apply_filter(Filter {
            region: Rect::from_min_max(pos2(0.0, 0.0), pos2(32.0, 64.0)),
            contrast: 0.0,
            ..Filter::default()
        });
    });

    let inside = capture.pixel(8, 32)[0];
    assert!(inside.abs_diff(128) <= 2, "the filtered half read {inside}");
    assert_eq!(capture.pixel(48, 32)[0], 255);
}
