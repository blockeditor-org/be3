use block_editor_beui::beui::Key;

use super::*;

#[test]
fn a_new_player_can_take_the_other_side() {
    let mut editor = editor(TIC_TAC_TOE.to_vec());
    editor.click("game.tile.0.0");
    editor.run();

    editor.click("game.player");
    editor.run();
    editor.text("New");
    editor.run();
    editor.key_press(Key::Enter);
    editor.run();

    editor.click("game.tile.1.1");
    editor.run();

    let played = moves(&editor);
    assert_eq!(played.len(), 2);
    assert_eq!(played[0].actor, ACCOUNT);
    assert_ne!(played[1].actor, ACCOUNT);
    assert_eq!(editor.label("game.history.0.1"), "b2");
    editor.snapshot("a_new_player_can_take_the_other_side");
}
