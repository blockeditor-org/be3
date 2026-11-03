use super::*;

#[test]
fn rects_with_no_place_sit_side_by_side_along_their_bottoms() {
    let rects = arrange(&[
        (vec2(1280.0, 720.0), None),
        (vec2(1920.0, 1080.0), None),
        (vec2(800.0, 600.0), None),
    ]);
    assert_eq!(
        rects,
        [
            Rect::from_min_size(pos2(0.0, 360.0), vec2(1280.0, 720.0)),
            Rect::from_min_size(pos2(1280.0, 0.0), vec2(1920.0, 1080.0)),
            Rect::from_min_size(pos2(3200.0, 480.0), vec2(800.0, 600.0)),
        ]
    );
}
