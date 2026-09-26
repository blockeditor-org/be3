use super::*;

#[test]
fn without_the_chrome_the_status_and_buttons_sit_under_the_board() {
    let mut editor = editor(CRAZY_8S.to_vec());

    editor.set_chrome(false);
    editor.run();

    assert!(!editor.shown("game.player"));
    assert_eq!(editor.label("game.description"), "Join the game");
    editor.snapshot("without_the_chrome_the_status_and_buttons_sit_under_the_board");

    editor.click("game.action.0");
    editor.run();

    let played = moves(&editor);
    assert_eq!(played.len(), 1);
    assert_eq!(played[0].actor, ACCOUNT);
}
