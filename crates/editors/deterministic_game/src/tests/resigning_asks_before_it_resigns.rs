use super::*;

#[test]
fn resigning_asks_before_it_resigns() {
    let actions = played(CHESS, &[(ACCOUNT, "e4"), (OPPONENT, "e5")]);
    let resign = offered(CHESS, &actions, ACCOUNT)
        .iter()
        .position(|option| option.label == "Resign")
        .expect("a seated player can resign");
    let mut editor = editor_after(CHESS.to_vec(), actions.clone());

    editor.click(&format!("game.control.{resign}"));
    editor.run();
    assert_eq!(moves(&editor).len(), actions.len());
    editor.snapshot("resigning_asks_before_it_resigns");

    editor.click(&format!("game.control.{resign}.confirm"));
    editor.run();

    assert_eq!(moves(&editor).len(), actions.len() + 1);
    assert!(editor.shown("game.banner"));
}
