use super::*;

#[test]
fn the_history_lists_every_move_and_steps_back_through_it() {
    let actions = played(TIC_TAC_TOE, &[(ACCOUNT, "b2"), (OPPONENT, "a3")]);
    let mut editor = editor_after(TIC_TAC_TOE.to_vec(), actions.clone());

    assert_eq!(editor.label("game.history.0.0"), "b2");
    assert_eq!(editor.label("game.history.0.1"), "a3");

    editor.click("game.history.0.0");
    editor.run();

    editor.snapshot("the_history_steps_back_to_the_first_move");
    editor.click("game.tile.2.2");
    editor.run();
    assert_eq!(moves(&editor).len(), actions.len());

    editor.click("game.history.next");
    editor.run();

    editor.click("game.tile.2.2");
    editor.run();
    assert_eq!(moves(&editor).len(), actions.len() + 1);
    assert_eq!(editor.label("game.history.1.0"), "c1");
}
