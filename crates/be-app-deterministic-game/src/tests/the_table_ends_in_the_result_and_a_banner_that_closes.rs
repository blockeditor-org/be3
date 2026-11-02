use super::*;

#[test]
fn the_table_ends_in_the_result_and_a_banner_that_closes() {
    let actions = played(
        CHESS,
        &[
            (ACCOUNT, "f3"),
            (OPPONENT, "e5"),
            (ACCOUNT, "g4"),
            (OPPONENT, "Qh4"),
        ],
    );
    let mut editor = editor_after(CHESS.to_vec(), actions);

    assert_eq!(editor.label("game.history.0.0"), "f3");
    assert_eq!(editor.label("game.history.0.1"), "e5");
    assert_eq!(editor.label("game.history.1.0"), "g4");
    assert_eq!(editor.label("game.history.1.1"), "Qh4#");
    assert_eq!(
        editor.label("game.result"),
        "0-1 You lose - Black wins by checkmate"
    );
    assert!(editor.shown("game.banner"));
    editor.snapshot("the_table_ends_in_the_result_and_a_banner_that_closes");

    editor.click("game.banner.close");
    editor.run();

    assert!(!editor.shown("game.banner"));
    assert!(editor.shown("game.result"));
}
