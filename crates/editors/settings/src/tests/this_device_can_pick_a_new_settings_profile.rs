use super::*;
use crate::app::SettingsProfileApp;
use block_editor_beui::be_block::SettingsProfileContent;
use block_editor_beui::{BlockInfo, BlockParent};

#[test]
fn this_device_can_pick_a_new_settings_profile() {
    let mut settings = editor();
    settings.click("settings.devices");
    settings.run();
    settings.click("settings.new-profile");
    settings.run();
    let [profile] = settings.content::<SettingsContent>(None).root().profiles()[..] else {
        panic!("one profile is listed");
    };
    let info = settings.store().block(profile).expect("the profile exists");
    assert_eq!(info.block_type, SettingsProfileContent::CONTENT_TYPE);
    assert_eq!(info.name().as_deref(), Some("Profile 1"));
    settings.snapshot("a_new_settings_profile_is_listed");

    let (root, client) = (Uuid::from_u128(0xd0_0001), Uuid::from_u128(0xd0_0002));
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(client);
    let mut chosen = BeuiTest::<SettingsProfileApp>::new(OwnedEditor::new(host, profile));
    chosen.hold(None, SettingsProfileContent::default());
    let store = chosen.store();
    store.add_block(BlockInfo::new(
        root,
        SettingsContent::CONTENT_TYPE,
        BlockParent::Root,
    ));
    store.add_block(BlockInfo::new(
        profile,
        SettingsProfileContent::CONTENT_TYPE,
        BlockParent::Block(root),
    ));
    store.hold(Some(root), SettingsContent::default());
    chosen.run();

    chosen.click("settings-profile.use");
    chosen.run();
    let held: SettingsContent = chosen.content(Some(root));
    assert_eq!(held.root().device_profiles.get(&client), Some(&profile));
    assert!(
        chosen
            .document()
            .find_test_id("settings-profile.use")
            .is_none(),
        "a profile in use is not offered again"
    );
    chosen.snapshot("a_settings_profile_used_on_this_device");
}
