use super::*;

#[test]
fn checkers_jumps_are_compulsory_and_chain_to_the_end() {
    let position = board(&[
        ("a1", &MAN, Side::First),
        ("g1", &MAN, Side::First),
        ("b2", &MAN, Side::Second),
        ("b4", &MAN, Side::Second),
    ]);

    let legal = position.legal(Side::First, true);

    assert_eq!(legal.len(), 1);
    assert_eq!(legal[0].from, square("a1"));
    assert_eq!(legal[0].via, vec![square("c3")]);
    assert_eq!(legal[0].to, square("a5"));
    assert_eq!(legal[0].captures, vec![square("b2"), square("b4")]);
}
