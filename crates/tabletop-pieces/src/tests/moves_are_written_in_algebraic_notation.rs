use super::*;

#[test]
fn moves_are_written_in_algebraic_notation() {
    let position = board(&[
        ("e1", &KING, Side::First),
        ("b1", &KNIGHT, Side::First),
        ("f3", &KNIGHT, Side::First),
        ("a1", &ROOK, Side::First),
        ("a5", &ROOK, Side::First),
        ("e8", &KING, Side::Second),
        ("d2", &KNIGHT, Side::Second),
    ]);

    let moves = written(&position, Side::First);

    assert!(moves.contains(&"Nbxd2".to_owned()));
    assert!(moves.contains(&"Nfxd2".to_owned()));
    assert!(moves.contains(&"R1a3".to_owned()));
    assert!(moves.contains(&"R5a3".to_owned()));
    assert!(moves.contains(&"Kxd2".to_owned()));
}
