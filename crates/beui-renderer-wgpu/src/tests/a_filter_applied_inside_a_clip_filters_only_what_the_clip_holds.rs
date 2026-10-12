use super::*;

#[test]
fn a_filter_applied_inside_a_clip_filters_only_what_the_clip_holds() {
    let half = |left: f32| Rect::from_min_max(pos2(left, 0.0), pos2(left + 32.0, 64.0));
    let capture = capture(Color32::BLACK, |painter| {
        painter.rect_filled(half(0.0), 0.0, Color32::WHITE);
        painter.rect_filled(half(0.0), 0.0, Color32::WHITE);
        painter.ctx().clipped(half(32.0), || {
            painter.rect_filled(half(32.0), 0.0, Color32::WHITE);
            painter.ctx().apply_filter(Filter {
                region: everything(),
                contrast: 0.0,
                ..Filter::default()
            });
        });
    });

    let [left, ..] = capture.pixel(8, 32);
    assert_eq!(
        left, 255,
        "the filter reached outside the clip it was applied in"
    );
    let [right, ..] = capture.pixel(48, 32);
    assert!(right.abs_diff(128) <= 2, "the clipped half read {right}");
}
