use super::*;

#[test]
fn a_bounded_renderer_filters_and_paints_only_within_its_bounds() {
    let mut target = Target::new();
    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(everything(), 0.0, Color32::WHITE);
    });

    target.bound([0, 0, 32, 64]);
    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(everything(), 0.0, Color32::BLACK);
        painter.ctx().apply_filter(Filter {
            region: everything(),
            contrast: 0.0,
            ..Filter::default()
        });
    });
    let capture = target.read();

    let inside = capture.pixel(8, 32)[0];
    assert!(
        inside.abs_diff(128) <= 2,
        "the filter applies within the bounds, which read {inside}"
    );
    assert_eq!(
        capture.pixel(48, 32),
        [255, 255, 255, 255],
        "what lies outside the bounds is left as it was"
    );
}
