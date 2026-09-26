use super::*;

#[test]
fn clicking_the_draw_pile_draws_a_card() {
    let actions = dealt();
    let draw = offered(CRAZY_8S, &actions, ACCOUNT)
        .into_iter()
        .find(|option| option.label == "Draw a card")
        .expect("drawing is always offered at the start of a turn");
    let mut editor = editor_after(CRAZY_8S.to_vec(), actions.clone());

    editor.click("game.pile.0");
    editor.run();

    let played = moves(&editor);
    assert_eq!(played.len(), actions.len() + 1);
    assert_eq!(played.last().map(|last| &last.action), Some(&draw.effect));
}
