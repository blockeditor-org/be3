use super::*;

#[test]
fn fools_mate_ends_the_game_and_the_history_reads_in_algebraic_notation() {
    let actions = played(&[
        (FIRST, "f3"),
        (SECOND, "e5"),
        (FIRST, "g4"),
        (SECOND, "Qh4"),
    ]);

    let screen = show(&actions, FIRST);

    assert_eq!(screen.description, "You lose - Black wins by checkmate");
    assert!(screen.actions.is_empty());
    let history: Vec<&str> = screen
        .history
        .iter()
        .map(|turn| turn.description.as_str())
        .collect();
    assert_eq!(history, ["f3", "e5", "g4", "Qh4#"]);
    assert_eq!(screen.history[3].actor, SECOND);
    assert_eq!(screen.history[3].column, Some(1));
    assert_eq!(*screen.columns, ["White", "Black"]);
    assert_eq!(
        screen.ending.and_then(|ending| ending.score).as_deref(),
        Some("0-1")
    );
    assert_eq!(screen.history[3].entry, 3);
    assert_eq!(show(&actions, SECOND).description, "You win by checkmate");
}
