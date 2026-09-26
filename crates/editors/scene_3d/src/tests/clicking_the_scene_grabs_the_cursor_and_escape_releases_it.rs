use super::*;

#[test]
fn clicking_the_scene_grabs_the_cursor_and_escape_releases_it() {
    let (mut editor, host) = editor();
    assert!(!host.cursor_grabbed());
    editor.snapshot("clicking_the_scene_grabs_the_cursor_and_escape_releases_it");

    editor.click("scene.viewport");
    editor.run();
    assert!(host.cursor_grabbed());
    assert_eq!(editor.take_cursor_grab(), Some(true));

    editor.key_press(beui::Key::Escape);
    editor.run();
    assert!(!host.cursor_grabbed());
}
