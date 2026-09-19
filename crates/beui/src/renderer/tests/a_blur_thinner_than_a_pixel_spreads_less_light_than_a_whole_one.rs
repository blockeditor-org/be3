use super::*;

#[test]
fn a_blur_thinner_than_a_pixel_spreads_less_light_than_a_whole_one() {
    let square = Rect::from_min_max(pos2(24.0, 24.0), pos2(40.0, 40.0));
    let spread_at = |radius: f32| {
        capture(Color32::BLACK, |painter| {
            painter.rect_filled(square, 0.0, Color32::WHITE);
            painter.ctx().apply_filter(Filter {
                region: everything(),
                blur: radius,
                ..Filter::default()
            });
        })
        .pixel(32, 23)[0]
    };

    let sharp = spread_at(0.0);
    let quarter = spread_at(0.25);
    let whole = spread_at(1.0);

    assert_eq!(sharp, 0);
    assert!(quarter > sharp, "a quarter of a pixel spread {quarter}");
    assert!(
        quarter < whole,
        "a quarter of a pixel spread {quarter} against {whole} for a whole one"
    );
}
