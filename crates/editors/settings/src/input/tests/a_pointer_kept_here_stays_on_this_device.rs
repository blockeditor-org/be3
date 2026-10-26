use super::*;
use block_editor_beui::be_block::{
    InputSettings, Root, Settings, SettingsContent, SettingsProfile, SettingsProfileContent,
};
use block_editor_beui::beui::Vec2;
use block_editor_beui::{BlockInfo, BlockParent};

const TAP: &str = "input.pointer.tap-to-click";

#[test]
fn a_pointer_kept_here_stays_on_this_device() {
    let (block, client, here) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let (settings, profile) = (Uuid::new_v4(), Uuid::new_v4());
    let host = EditorHost::default();
    host.set_watched::<InputDevicesWatch>(&vec![touchpad()]);
    host.set_editable(true);
    host.set_client_id(client);
    let mut editor = BeuiTest::<InputSettingsApp>::new(OwnedEditor::new(host, block));
    editor.hold(None, InputSettingsContent::default());
    editor.set_size(Vec2::new(800.0, 1200.0));
    let store = editor.store();
    store.add_block(BlockInfo::new(
        settings,
        Settings::CONTENT_TYPE,
        BlockParent::Root,
    ));
    let mut held = SettingsContent::default();
    held.apply(&Settings::set_kind(InputSettings::CONTENT_TYPE, block));
    held.apply(&Settings::add_profile(client, profile));
    store.hold(Some(settings), held);
    let mut own = SettingsProfileContent::default();
    own.apply(&SettingsProfile::set_kind(
        InputSettings::CONTENT_TYPE,
        here,
    ));
    store.hold(Some(profile), own);
    store.hold(Some(here), InputSettingsContent::default());
    editor.run();
    choose_pointer(&mut editor, 1);
    let tap = |editor: &BeuiTest<InputSettingsApp>, layer: Option<Uuid>| {
        editor
            .content::<InputSettingsContent>(layer)
            .root()
            .device_pointer(&touchpad_identity())
            .tap_to_click
    };

    editor.click(TAP);
    editor.run();
    assert_eq!(
        tap(&editor, None),
        Some(false),
        "a change goes to every device"
    );
    assert_eq!(tap(&editor, Some(here)), None);

    editor.click(format!("{TAP}.menu"));
    editor.run();
    editor.click(format!("{TAP}.keep"));
    editor.run();
    assert_eq!(tap(&editor, Some(here)), Some(false));

    editor.click(TAP);
    editor.run();
    assert_eq!(
        tap(&editor, Some(here)),
        Some(true),
        "a kept pointer changes here"
    );
    assert_eq!(tap(&editor, None), Some(false));
}
