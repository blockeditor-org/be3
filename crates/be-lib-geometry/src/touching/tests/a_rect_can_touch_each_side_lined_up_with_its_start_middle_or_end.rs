use super::*;

#[test]
fn a_rect_can_touch_each_side_lined_up_with_its_start_middle_or_end() {
    let laptop = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0));
    let found = places(vec2(400.0, 300.0), &[laptop]);
    assert_eq!(
        found.len(),
        12,
        "three lined-up places on each of four sides"
    );
    let at = |side: Side, line: Line| {
        found
            .iter()
            .find(|place| place.side == side && place.line == line)
            .map(|place| place.rect.min)
    };
    assert_eq!(at(Side::Right, Line::Start), Some(pos2(1000.0, 0.0)));
    assert_eq!(at(Side::Left, Line::Middle), Some(pos2(-400.0, 100.0)));
    assert_eq!(at(Side::Above, Line::End), Some(pos2(600.0, -300.0)));

    let monitor = Rect::from_min_size(pos2(1000.0, 0.0), vec2(500.0, 500.0));
    let found = places(vec2(500.0, 500.0), &[laptop, monitor]);
    assert!(
        found
            .iter()
            .all(|place| !overlaps_any(place.rect, &[laptop, monitor])),
        "no place overlaps a rect already placed"
    );
    assert_eq!(
        found
            .iter()
            .filter(|place| place.rect.min == pos2(1500.0, 0.0))
            .count(),
        1,
        "a place two sides name is listed once"
    );
}
