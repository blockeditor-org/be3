use super::*;

#[test]
fn a_rotated_rectangle_covers_the_corners_it_turned_onto() {
    let rect = Rect::from_min_max(pos2(22.0, 30.0), pos2(42.0, 34.0));
    let upright = capture(Color32::BLACK, |painter| {
        painter.rect_filled(rect, 0.0, Color32::WHITE);
    });
    let turned = capture(Color32::BLACK, |painter| {
        painter
            .rotated(rect.center(), std::f32::consts::FRAC_PI_2)
            .rect_filled(rect, 0.0, Color32::WHITE);
    });

    assert_eq!(upright.pixel(24, 32), [255, 255, 255, 255]);
    assert_eq!(upright.pixel(32, 24), [0, 0, 0, 255]);
    assert_eq!(turned.pixel(32, 24), [255, 255, 255, 255]);
    assert_eq!(turned.pixel(24, 32), [0, 0, 0, 255]);
}
