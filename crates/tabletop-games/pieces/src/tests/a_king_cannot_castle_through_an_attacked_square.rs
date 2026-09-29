use super::*;

#[test]
fn a_king_cannot_castle_through_an_attacked_square() {
    let free = board(&[
        ("e1", &KING, Side::First),
        ("a1", &ROOK, Side::First),
        ("h1", &ROOK, Side::First),
        ("e8", &KING, Side::Second),
    ]);
    let moves = written(&free, Side::First);
    assert!(moves.contains(&"O-O".to_owned()));
    assert!(moves.contains(&"O-O-O".to_owned()));

    let guarded = board(&[
        ("e1", &KING, Side::First),
        ("a1", &ROOK, Side::First),
        ("h1", &ROOK, Side::First),
        ("e8", &KING, Side::Second),
        ("f8", &ROOK, Side::Second),
    ]);
    let moves = written(&guarded, Side::First);
    assert!(!moves.contains(&"O-O".to_owned()));
    assert!(moves.contains(&"O-O-O".to_owned()));

    let mut castled = free.clone();
    let legal = free.legal(Side::First, false);
    castled.apply(find(&legal, "e1", "g1").expect("the king castles short"));
    assert_eq!(
        castled.at(square("f1")).map(|man| man.piece.name()),
        Some("rook")
    );
    assert!(castled.at(square("h1")).is_none());
}
