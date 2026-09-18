use super::*;

#[test]
fn the_replace_panel_goes_away_with_the_chrome() {
    let (mut test, editor) = editor(b"not a wasm module".to_vec());

    assert!(test.shown("game-module.replace"));
    let with_panel = editor.content_rect();

    editor.host().set_chrome_shown(false);
    test.run();

    let without_panel = editor.content_rect();
    assert!(with_panel.right() < without_panel.right());
    assert_eq!(without_panel, test.rect());
}
