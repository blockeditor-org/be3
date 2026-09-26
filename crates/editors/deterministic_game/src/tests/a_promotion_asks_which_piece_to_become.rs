use super::*;

#[test]
fn a_promotion_asks_which_piece_to_become() {
    let actions = played(
        CHESS,
        &[
            (ACCOUNT, "a4"),
            (OPPONENT, "b5"),
            (ACCOUNT, "axb5"),
            (OPPONENT, "a6"),
            (ACCOUNT, "bxa6"),
            (OPPONENT, "Bb7"),
            (ACCOUNT, "axb7"),
            (OPPONENT, "Nc6"),
        ],
    );
    let knight = offered(CHESS, &actions, ACCOUNT)
        .iter()
        .position(|option| option.label == "b8=N")
        .expect("the pawn can become a knight");
    let mut editor = editor_after(CHESS.to_vec(), actions.clone());

    let from = editor.rect_of("game.tile.1.1").center();
    let to = editor.rect_of("game.tile.1.0").center();
    editor.drag(from, to);
    editor.run();
    assert_eq!(moves(&editor).len(), actions.len());
    editor.snapshot("a_promotion_asks_which_piece_to_become");

    editor.click(&format!("game.choice.{knight}"));
    editor.run();

    let played = moves(&editor);
    assert_eq!(played.len(), actions.len() + 1);
    assert_eq!(
        played.last().map(|last| &last.action),
        Some(&offered(CHESS, &actions, ACCOUNT)[knight].effect)
    );
}
