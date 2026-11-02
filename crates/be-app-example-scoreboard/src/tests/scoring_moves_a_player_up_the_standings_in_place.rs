use super::*;

#[test]
fn scoring_moves_a_player_up_the_standings_in_place() {
    let mut board = Harness::new(&[("Ada", 2), ("Bob", 1), ("Cy", 0)]);
    let bob = board.id("Bob");
    let row = board
        .editor
        .document()
        .find_test_id(&format!("scoreboard.player.{bob}.score"));
    assert_eq!(
        board.shown(&format!("scoreboard.player.{bob}.place")),
        "\"#2\""
    );

    board.click(&format!("scoreboard.player.{bob}.plus"));
    board.click(&format!("scoreboard.player.{bob}.plus"));

    assert_eq!(
        board.shown(&format!("scoreboard.player.{bob}.place")),
        "\"#1\""
    );
    assert_eq!(
        board.shown(&format!("scoreboard.player.{bob}.score")),
        "\"3\""
    );
    assert_eq!(
        board
            .editor
            .document()
            .find_test_id(&format!("scoreboard.player.{bob}.score")),
        row,
        "moving up the standings keeps the row rather than building a new one"
    );
    board
        .editor
        .snapshot("scoring_moves_a_player_up_the_standings_in_place");
}
