use super::*;

#[test]
fn a_player_joins_by_clicking_the_deck_even_without_the_chrome() {
    let mut editor = editor(CRAZY_8S.to_vec());

    editor.set_chrome(false);
    editor.run();

    assert!(!editor.shown("game.player"));
    editor.snapshot("a_player_joins_by_clicking_the_deck_even_without_the_chrome");

    editor.click("game.pile.0");
    editor.run();

    let played = moves(&editor);
    assert_eq!(played.len(), 1);
    assert_eq!(played[0].actor, ACCOUNT);
}
