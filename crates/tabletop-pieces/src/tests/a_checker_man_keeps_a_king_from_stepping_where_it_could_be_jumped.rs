use super::*;

#[test]
fn a_checker_man_keeps_a_king_from_stepping_where_it_could_be_jumped() {
    let position = board(&[("e5", &KING, Side::First), ("e7", &MAN, Side::Second)]);

    let legal = position.legal(Side::First, false);

    assert!(find(&legal, "e5", "e6").is_some());
    assert!(find(&legal, "e5", "d6").is_none());
    assert!(find(&legal, "e5", "f6").is_none());
}
