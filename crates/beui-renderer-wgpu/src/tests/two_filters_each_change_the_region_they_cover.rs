use super::*;

#[test]
fn two_filters_each_change_the_region_they_cover() {
    let half = |left: f32| Rect::from_min_max(pos2(left, 0.0), pos2(left + 32.0, 64.0));
    let capture = capture(Color32::BLACK, |painter| {
        painter.rect_filled(everything(), 0.0, Color32::WHITE);
        painter.ctx().apply_filter(Filter {
            region: half(0.0),
            contrast: 0.0,
            ..Filter::default()
        });
        painter.ctx().apply_filter(Filter {
            region: half(32.0),
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
