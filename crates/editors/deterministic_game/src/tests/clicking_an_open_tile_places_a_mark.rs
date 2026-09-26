use super::*;

#[test]
fn clicking_an_open_tile_places_a_mark() {
    let mut editor = editor(TIC_TAC_TOE.to_vec());
    editor.record();

    editor.click("game.tile.1.1");
    editor.run();

    let played = moves(&editor);
    assert_eq!(played.len(), 1);
    assert_eq!(played[0].actor, ACCOUNT);
    assert_eq!(editor.label("game.history.0.0"), "b2");
    editor.record();
    editor.snapshot("clicking_an_open_tile_places_a_mark");
}
