use super::*;

#[test]
fn a_man_jumps_a_chess_piece_when_it_can() {
    let actions = played(&[(FIRST, "e4"), (SECOND, "de5"), (FIRST, "d4")]);

    assert_eq!(labels(&actions, SECOND), ["exc3", "Resign"]);
}
