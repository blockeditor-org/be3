use be_sdk_beui::FocusedBlock;
use be_sdk_beui::be_block::{OfflineSettingsContent, WORKSPACE_TYPE, organization_settings};

use super::*;
use crate::root::{DEVICES_EDITOR, ORGANIZATION_EDITOR, STORAGE_EDITOR};

#[test]
fn each_setting_opens_as_a_page_beside_the_list() {
    let organization = Uuid::from_u128(0x0e0f);
    let mut settings = editor_in::<SettingsRoot>(Uuid::nil(), organization);
    settings.set_size(Vec2::new(300.0, 760.0));
    let block = settings.block_id().expect("the settings are a block");
    let [tablet, team, notes] = [1, 2, 3].map(|id| Uuid::from_u128(0x0a00 + id));
    let store = settings.store();
    let mut profile = BlockInfo::new(
        tablet,
        SettingsProfileContent::CONTENT_TYPE,
        BlockParent::Block(block),
    );
    profile.set_name("PineNote");
    store.add_block(profile);
    let mut own = SettingsProfileContent::default();
    own.apply(&SettingsProfile::set_class(Some(DeviceClass::EpaperTablet)));
    store.hold(Some(tablet), own);
    for (id, name) in [(team, "Team"), (notes, "Notes")] {
        let mut info = BlockInfo::new(id, WORKSPACE_TYPE, BlockParent::Root);
        info.set_name(name);
        store.add_block(info);
    }
    let policies = organization_settings(organization);
    store.add_block(BlockInfo::new(
        policies,
        SettingsContent::CONTENT_TYPE,
        BlockParent::Root,
    ));
    store.hold(Some(policies), SettingsContent::default());
    let mut held = settings.content::<SettingsContent>(None);
    held.apply(&Settings::list_profile(tablet));
    held.apply(&Settings::add_profile(Uuid::from_u128(0x5e77_0002), tablet));
    settings.hold(None, held);
    settings.run();
    settings.take_opens();

    settings.click("settings.devices");
    settings.run();
    settings.click("settings.organization");
    settings.run();
    settings.click(&format!("settings.profile.{tablet}"));
    settings.run();
    settings.click("settings.offline");
    settings.run();
    let opens: Vec<(Uuid, Option<Uuid>)> = settings
        .take_opens()
        .into_iter()
        .map(|open| (open.block_id, open.editor))
        .collect();
    let [offline] = entries(&settings, OfflineSettingsContent::CONTENT_TYPE)[..] else {
        panic!("opening Offline makes its block once");
    };
    assert_eq!(
        opens,
        vec![
            (block, Some(DEVICES_EDITOR)),
            (policies, Some(ORGANIZATION_EDITOR)),
            (tablet, None),
            (offline, None),
        ],
        "every row opens a page instead of changing anything in the list"
    );
    assert!(
        settings
            .label(&format!("settings.profile.{tablet}"))
            .ends_with("PineNote This device · E-paper tablet"),
        "a profile says which device uses it and what kind it is"
    );

    settings.host().report_focus(FocusedBlock {
        block_id: Some(tablet),
        block_type: SettingsProfileContent::CONTENT_TYPE,
        editor: None,
        via: Vec::new(),
    });
    settings.run();
    settings.snapshot("the_list_marks_the_page_shown_beside_it");

    settings.host().report_focus(FocusedBlock {
        block_id: Some(block),
        block_type: SettingsContent::CONTENT_TYPE,
        editor: Some(STORAGE_EDITOR),
        via: Vec::new(),
    });
    settings.run();
    settings.snapshot("the_list_marks_storage_not_devices_when_storage_is_shown");
}
