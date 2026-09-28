use super::*;

#[test]
fn reducing_contrast_pulls_the_filtered_region_toward_grey() {
    let capture = capture(Color32::BLACK, |painter| {
        painter.rect_filled(
            Rect::from_min_max(pos2(0.0, 0.0), pos2(32.0, 64.0)),
            0.0,
            Color32::WHITE,
        );
        painter.ctx().apply_filter(Filter {
            region: everything(),
            contrast: 0.0,
            ..Filter::default()
        });
    });

    for (x, y) in [(8, 32), (48, 32)] {
        let [red, green, blue, _] = capture.pixel(x, y);
        for channel in [red, green, blue] {
            assert!(
                channel.abs_diff(128) <= 2,
                "the pixel at {x}, {y} read {red}, {green}, {blue}"
            );
        }
    }
}
