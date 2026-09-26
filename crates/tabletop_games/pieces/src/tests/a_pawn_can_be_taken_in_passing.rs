use super::*;

#[test]
fn a_pawn_can_be_taken_in_passing() {
    let mut position = board(&[
        ("e1", &KING, Side::First),
        ("e5", &PAWN, Side::First),
        ("e8", &KING, Side::Second),
        ("d7", &PAWN, Side::Second),
    ]);
    let legal = position.legal(Side::Second, false);
    position.apply(find(&legal, "d7", "d5").expect("a fresh pawn steps two squares"));

    let legal = position.legal(Side::First, false);
    let taking = find(&legal, "e5", "d6").expect("the pawn that passed can be taken");
    assert_eq!(algebraic(&position, taking, &legal), "exd6");
    position.apply(taking);

    assert!(position.at(square("d5")).is_none());
}
