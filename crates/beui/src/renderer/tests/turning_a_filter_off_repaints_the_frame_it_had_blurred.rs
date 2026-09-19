use super::*;

#[test]
fn turning_a_filter_off_repaints_the_frame_it_had_blurred() {
    let square = Rect::from_min_max(pos2(24.0, 24.0), pos2(40.0, 40.0));
    let mut target = Target::new();

    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(square, 0.0, Color32::WHITE);
        painter.ctx().apply_filter(Filter {
            region: everything(),
            blur: 12.0,
            ..Filter::default()
        });
    });
    let blurred = target.read();
    assert!(blurred.pixel(32, 18)[0] > 20);

    let corner = Rect::from_min_max(pos2(0.0, 0.0), pos2(4.0, 4.0));
    target.draw(
        Color32::BLACK,
        Repaint::Region {
            region: corner,
            background: Color32::BLACK,
        },
        |painter| painter.rect_filled(square, 0.0, Color32::WHITE),
    );
    let sharp = target.read();

    assert_eq!(sharp.pixel(32, 18)[0], 0, "the blur outlived the filter");
    assert_eq!(sharp.pixel(32, 32)[0], 255);
}
