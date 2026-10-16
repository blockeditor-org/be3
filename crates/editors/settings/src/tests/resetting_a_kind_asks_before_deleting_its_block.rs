use super::*;
use block_editor_beui::BlockCommand;

#[test]
fn resetting_a_kind_asks_before_deleting_its_block() {
    let mut editor = editor();
    editor.click("settings.ui-settings");
    editor.run();
    let [ui] = entries(&editor, UiSettingsContent::CONTENT_TYPE)[..] else {
        panic!("opening the appearance settings makes one block");
    };
    editor.take_block_commands();

    editor.click("settings.ui-settings.reset");
    editor.run();
    editor.snapshot("resetting_a_kind_asks_before_deleting_its_block");
    assert!(
        editor.take_block_commands().is_empty(),
        "the reset asks first rather than deleting"
    );

    editor.click("settings.reset.cancel");
    editor.run();
    assert!(editor.take_block_commands().is_empty());

    editor.click("settings.ui-settings.reset");
    editor.run();
    editor.click("settings.reset.confirm");
    editor.run();
    let deleted = editor
        .take_block_commands()
        .into_iter()
        .any(|(id, command)| id == ui && matches!(command, BlockCommand::Delete { .. }));
    assert!(deleted, "confirming deletes the block the row points at");
}
