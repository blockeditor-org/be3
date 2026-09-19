use super::*;

#[test]
fn a_blur_repaints_the_light_it_spreads_outside_the_damaged_region() {
    let blur = || Filter {
        region: everything(),
        blur: 4.0,
        ..Filter::default()
    };
    let marker = Rect::from_min_max(pos2(0.0, 0.0), pos2(8.0, 64.0));
    let bar = Rect::from_min_max(pos2(28.0, 0.0), pos2(44.0, 64.0));
    let mut target = Target::new();

    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(marker, 0.0, Color32::WHITE);
        painter.ctx().apply_filter(blur());
    });

    target.draw(
        Color32::BLACK,
        Repaint::Region {
            region: bar,
            background: Color32::BLACK,
        },
        |painter| {
            painter.rect_filled(bar, 0.0, Color32::WHITE);
            painter.ctx().apply_filter(blur());
        },
    );
    let capture = target.read();

    let spread = capture.pixel(22, 32)[0];
    assert!(
        spread > 5,
        "the blur stopped at the damaged region with {spread}"
    );
    let kept = capture.pixel(2, 32)[0];
    assert!(
        kept > 100,
        "the frame repainted past its damage, reading {kept}"
    );
}
