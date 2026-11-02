use super::*;

#[test]
fn resetting_puts_every_score_back_to_zero() {
    let mut board = Harness::new(&[("Ada", 4), ("Bob", -1)]);

    board.click("scoreboard.reset");

    assert_eq!(
        board.scores(),
        [("Ada".to_owned(), 0), ("Bob".to_owned(), 0)]
    );
}
