use super::*;

#[test]
fn the_replace_panel_goes_away_with_the_chrome() {
    let (mut test, editor) = editor();

    assert!(test.shown("audio.replace"));
    let with_panel = editor.content_rect();

    test.set_chrome(false);

    let without_panel = editor.content_rect();
    assert!(with_panel.right() < without_panel.right());
    assert_eq!(without_panel, test.rect());
}
