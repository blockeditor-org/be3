use super::*;

#[test]
fn a_punch_clears_what_it_covers() {
    let whole = Rect::from_min_size(Pos2::ZERO, vec2(SIZE as f32, SIZE as f32));
    let hole = Rect::from_min_max(pos2(16.0, 16.0), pos2(48.0, 48.0));
    let capture = capture(Color32::TRANSPARENT, |painter| {
        painter.rect_filled(whole, 0.0, Color32::WHITE);
        painter.punch(hole, 0.0);
    });

    assert_eq!(
        capture.pixel(32, 32),
        [0, 0, 0, 0],
        "the punched rectangle should have been cleared back to nothing"
    );
    assert_eq!(
        capture.pixel(4, 4)[3],
        255,
        "the fill outside the punched rectangle should have been left alone"
    );
}
