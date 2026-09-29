use super::*;

#[test]
fn a_knight_in_checkers_has_to_capture_like_everyone_else() {
    let position = board(&[
        ("b1", &KNIGHT, Side::First),
        ("g1", &MAN, Side::First),
        ("c3", &MAN, Side::Second),
    ]);

    let legal = position.legal(Side::First, true);

    assert_eq!(legal.len(), 1);
    assert_eq!(legal[0].from, square("b1"));
    assert_eq!(legal[0].captures, vec![square("c3")]);
}
