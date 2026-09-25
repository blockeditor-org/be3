use super::*;

#[test]
fn opening_ui_settings_creates_the_block_once() {
    let mut editor = editor();

    editor.click("settings.ui-settings");
    editor.run();
    assert_eq!(entries(&editor).len(), 1);

    editor.click("settings.ui-settings");
    editor.run();
    assert_eq!(entries(&editor).len(), 1);
    editor.snapshot("opening_ui_settings_creates_the_block_once");
}
