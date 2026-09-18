use super::*;

#[test]
fn a_layer_painted_above_a_filter_keeps_its_own_colours() {
    let capture = capture(Color32::BLACK, |painter| {
        painter.rect_filled(
            Rect::from_min_max(pos2(0.0, 0.0), pos2(32.0, 64.0)),
            0.0,
            Color32::from_rgb(255, 0, 0),
        );
        painter.ctx().apply_filter(Filter {
            region: everything(),
            vision: ColorVision::Achromatopsia,
            ..Filter::default()
        });
        painter.rect_filled(
            Rect::from_min_max(pos2(32.0, 0.0), pos2(64.0, 64.0)),
            0.0,
            Color32::from_rgb(0, 255, 0),
        );
    });

    let [red, green, blue, _] = capture.pixel(8, 32);
    assert!(
        red.abs_diff(green) <= 2 && green.abs_diff(blue) <= 2,
        "the filtered half read {red}, {green}, {blue}"
    );
    assert_eq!(capture.pixel(48, 32), [0, 255, 0, 255]);
}
