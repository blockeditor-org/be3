use super::*;

#[test]
fn a_blurred_region_spreads_light_past_the_shape_that_made_it() {
    let square = Rect::from_min_max(pos2(24.0, 24.0), pos2(40.0, 40.0));
    let mut target = Target::new();

    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(square, 0.0, Color32::WHITE);
    });
    let sharp = target.read();

    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(square, 0.0, Color32::WHITE);
        painter.ctx().apply_filter(Filter {
            region: everything(),
            blur: 12.0,
            ..Filter::default()
        });
    });
    let blurred = target.read();

    assert_eq!(sharp.pixel(32, 18)[0], 0);
    let spread = blurred.pixel(32, 18)[0];
    assert!(spread > 20, "the blur reached the pixel with only {spread}");
    let centre = blurred.pixel(32, 32)[0];
    assert!(centre < 255, "the blur left the centre at {centre}");
}
