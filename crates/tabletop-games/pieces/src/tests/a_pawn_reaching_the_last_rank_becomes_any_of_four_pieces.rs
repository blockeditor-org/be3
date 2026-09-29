use super::*;

#[test]
fn a_pawn_reaching_the_last_rank_becomes_any_of_four_pieces() {
    let position = board(&[
        ("a1", &KING, Side::First),
        ("b7", &PAWN, Side::First),
        ("h8", &KING, Side::Second),
    ]);

    let moves = written(&position, Side::First);

    for promotion in ["b8=B", "b8=N", "b8=Q", "b8=R"] {
        assert!(
            moves.contains(&promotion.to_owned()),
            "{promotion} is missing"
        );
    }
}
