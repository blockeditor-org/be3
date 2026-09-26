use super::*;

#[test]
fn checkers_moves_are_numbered_from_the_first_players_side() {
    let mut position = Position::empty(8, 8);
    crate::checkers::army(&mut position, Side::First, 3);
    crate::checkers::army(&mut position, Side::Second, 3);

    let legal = position.legal(Side::First, true);
    let mut moves: Vec<String> = legal.iter().map(|step| numbered(&position, step)).collect();
    moves.sort();

    assert_eq!(
        moves,
        ["10-14", "10-15", "11-15", "11-16", "12-16", "9-13", "9-14"]
    );
}
