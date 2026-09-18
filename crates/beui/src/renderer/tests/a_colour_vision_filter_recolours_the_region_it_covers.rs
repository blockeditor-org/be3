use super::*;

#[test]
fn a_colour_vision_filter_recolours_the_region_it_covers() {
    let capture = capture(Color32::BLACK, |painter| {
        painter.rect_filled(everything(), 0.0, Color32::from_rgb(255, 0, 0));
        painter.ctx().apply_filter(Filter {
            region: everything(),
            vision: ColorVision::Protanopia,
            ..Filter::default()
        });
    });

    let [red, green, blue, _] = capture.pixel(32, 32);
    assert!(red < 200, "protanopia left the red at {red}");
    assert!(green > 40, "protanopia left the green at {green}");
    assert!(blue < 40, "protanopia raised the blue to {blue}");
}
