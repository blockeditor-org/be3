use super::*;

#[test]
fn clearing_the_artwork_needs_the_dialog() {
    let (mut editor, _) = editor();

    assert!(!editor.shown("pixel-art.clear-apply"));
    editor.click("pixel-art.clear");
    editor.run();

    assert!(editor.shown("pixel-art.clear-apply"));
    editor.click("pixel-art.clear-cancel");
    editor.run();
    assert!(!editor.shown("pixel-art.clear-apply"));
}
