use super::*;

#[test]
fn a_rect_placed_apart_or_over_another_moves_to_the_nearest_edge() {
    let first = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0));
    assert_eq!(
        settle(Rect::from_min_size(pos2(900.0, 100.0), vec2(400.0, 300.0)), &[first]),
        Rect::from_min_size(pos2(1000.0, 100.0), vec2(400.0, 300.0)),
        "a rect over another's right side is pushed out of it"
    );
    assert_eq!(
        settle(Rect::from_min_size(pos2(300.0, 900.0), vec2(400.0, 300.0)), &[first]),
        Rect::from_min_size(pos2(300.0, 500.0), vec2(400.0, 300.0)),
        "a rect apart from every other is pulled to the nearest edge"
    );
    assert_eq!(
        settle(Rect::from_min_size(pos2(1100.0, 580.0), vec2(400.0, 300.0)), &[first]),
        Rect::from_min_size(pos2(1000.0, 436.0), vec2(400.0, 300.0)),
        "a rect that meets another only at a corner slides until they share an edge"
    );
    let second = Rect::from_min_size(pos2(1000.0, 0.0), vec2(500.0, 500.0));
    assert_eq!(
        settle(Rect::from_min_size(pos2(800.0, 0.0), vec2(400.0, 500.0)), &[first, second]),
        Rect::from_min_size(pos2(800.0, 500.0), vec2(400.0, 500.0)),
        "a rect over two others goes where it overlaps neither"
    );
}
