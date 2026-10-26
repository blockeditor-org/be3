use super::*;
use block_editor_beui::beui::Modifiers;

#[test]
fn clicking_the_scene_grabs_the_cursor_and_escape_releases_it() {
    let (mut editor, _host) = editor();
    assert!(!editor.pointer_locked());
    editor.snapshot("clicking_the_scene_grabs_the_cursor_and_escape_releases_it");

    editor.click("scene.viewport");
    editor.run();
    assert!(editor.pointer_locked());

    editor.key(beui::Key::Escape, Modifiers::NONE);
    editor.run();
    assert!(!editor.pointer_locked());
}
