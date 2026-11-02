use super::*;

#[test]
fn adding_a_player_puts_them_on_the_board() {
    let mut board = Harness::new(&[]);

    board.click("scoreboard.draft");
    board.type_text("Ada");
    board.click("scoreboard.add");

    assert_eq!(board.scores(), [("Ada".to_owned(), 0)]);
    assert_eq!(
        board.shown("scoreboard.summary"),
        "\"Ada is playing alone with 0\""
    );
}
