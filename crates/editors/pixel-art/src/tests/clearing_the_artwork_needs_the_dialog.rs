use super::*;

#[test]
fn clearing_the_artwork_needs_the_dialog() {
    let mut editor = editor();

    assert!(!editor.shown("pixel-art.clear.apply"));
    editor.click("pixel-art.clear");

    assert!(editor.shown("pixel-art.clear.apply"));
    editor.click("pixel-art.clear.cancel");
    assert!(!editor.shown("pixel-art.clear.apply"));
}
