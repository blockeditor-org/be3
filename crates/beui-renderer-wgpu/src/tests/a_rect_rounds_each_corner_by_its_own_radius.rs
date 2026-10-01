use super::*;
use beui_core::painter::Corners;

#[test]
fn a_rect_rounds_each_corner_by_its_own_radius() {
    let corners = Corners {
        top_left: 0.0,
        top_right: 16.0,
        bottom_right: 0.0,
        bottom_left: 16.0,
    };
    let capture = capture(Color32::BLACK, |painter| {
        painter.rect_filled(
            Rect::from_min_max(pos2(16.0, 16.0), pos2(48.0, 48.0)),
            corners,
            Color32::WHITE,
        );
    });

    assert_eq!(capture.pixel(16, 16), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(47, 47), [255, 255, 255, 255]);
    assert_eq!(capture.pixel(47, 16), [0, 0, 0, 255]);
    assert_eq!(capture.pixel(16, 47), [0, 0, 0, 255]);
    assert_eq!(capture.pixel(32, 32), [255, 255, 255, 255]);
}
