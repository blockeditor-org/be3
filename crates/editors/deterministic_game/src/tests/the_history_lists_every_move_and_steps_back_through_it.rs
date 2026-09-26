use super::*;

#[test]
fn the_history_lists_every_move_and_steps_back_through_it() {
    let actions = played(
        TIC_TAC_TOE,
        &[(ACCOUNT, "Row 2, column 2"), (OPPONENT, "Row 1, column 1")],
    );
    let mut editor = editor_after(TIC_TAC_TOE.to_vec(), actions.clone());

    assert_eq!(editor.label("game.history.0"), "1. Row 2, column 2 You");
    assert_eq!(editor.label("game.history.1"), "2. Row 1, column 1 Player 2");

    editor.click("game.history.0");
    editor.run();

    assert_eq!(
        editor.label("game.description"),
        "Looking back at move 1 of 2"
    );
    editor.snapshot("the_history_steps_back_to_the_first_move");
    editor.click("game.tile.2.2");
    editor.run();
    assert_eq!(moves(&editor).len(), actions.len());

    editor.click("game.history.next");
    editor.run();

    assert_eq!(editor.label("game.description"), "Your turn (X)");
    editor.click("game.tile.2.2");
    editor.run();
    assert_eq!(moves(&editor).len(), actions.len() + 1);
}
