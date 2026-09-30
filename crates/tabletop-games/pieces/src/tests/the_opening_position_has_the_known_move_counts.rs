use super::*;

#[test]
fn the_opening_position_has_the_known_move_counts() {
    let position = chess();

    assert_eq!(perft(&position, Side::First, 1), 20);
    assert_eq!(perft(&position, Side::First, 2), 400);
    assert_eq!(perft(&position, Side::First, 3), 8902);
}
