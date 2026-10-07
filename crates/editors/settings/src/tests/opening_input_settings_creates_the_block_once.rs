use super::*;

#[test]
fn opening_input_settings_creates_the_block_once() {
    let mut editor = editor();

    editor.click("settings.input-settings");
    editor.run();
    assert_eq!(
        entries(&editor, InputSettingsContent::CONTENT_TYPE).len(),
        1
    );

    editor.click("settings.input-settings");
    editor.run();
    assert_eq!(
        entries(&editor, InputSettingsContent::CONTENT_TYPE).len(),
        1
    );
    assert_eq!(entries(&editor, UiSettingsContent::CONTENT_TYPE).len(), 0);
}
