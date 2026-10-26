use block_editor_beui::be_block::{
    OfflinePolicy, OfflineSettings, OfflineSettingsContent, OrganizationPolicies,
    OrganizationPoliciesContent, WORKSPACE_TYPE, WhoMay, organization_settings,
};

use super::*;

fn named(id: Uuid, block_type: Uuid, parent: BlockParent, name: &str) -> BlockInfo {
    let mut info = BlockInfo::new(id, block_type, parent);
    info.set_name(name);
    info
}

#[test]
fn an_organizations_policies_show_to_its_administrators_and_fix_what_devices_keep_offline() {
    let organization = Uuid::from_u128(0x0e0f);
    let settings = organization_settings(organization);
    let [own, policies, offline, team, notes] =
        [1, 2, 3, 4, 5].map(|id| Uuid::from_u128(0x0e00 + id));
    let host = EditorHost::default();
    host.set_account_id(Uuid::nil());
    host.set_organization_id(organization);
    host.set_block_type(SettingsContent::CONTENT_TYPE);
    host.set_editable(true);
    host.set_client_id(Uuid::from_u128(0x0e10));
    let mut editor = BeuiTest::<SettingsApp>::new(OwnedEditor::new(host, own));
    editor.set_size(Vec2::new(720.0, 1400.0));
    let store = editor.store();
    store.add_block(BlockInfo::new(
        own,
        SettingsContent::CONTENT_TYPE,
        BlockParent::Root,
    ));
    store.add_block(named(team, WORKSPACE_TYPE, BlockParent::Root, "Team"));
    store.add_block(named(notes, WORKSPACE_TYPE, BlockParent::Root, "Notes"));
    store.add_block(BlockInfo::new(
        settings,
        SettingsContent::CONTENT_TYPE,
        BlockParent::Root,
    ));
    for (id, block_type) in [
        (policies, OrganizationPoliciesContent::CONTENT_TYPE),
        (offline, OfflineSettingsContent::CONTENT_TYPE),
    ] {
        store.add_block(BlockInfo::new(id, block_type, BlockParent::Block(settings)));
    }
    let mut held = SettingsContent::default();
    held.apply(&Settings::set_policy(
        OrganizationPoliciesContent::CONTENT_TYPE,
        policies,
    ));
    held.apply(&Settings::set_policy(
        OfflineSettingsContent::CONTENT_TYPE,
        offline,
    ));
    store.hold(Some(settings), held);
    let mut set = OrganizationPoliciesContent::default();
    set.apply(&OrganizationPolicies::set_history_days(Some(30)));
    store.hold(Some(policies), set);
    let mut kept = OfflineSettingsContent::default();
    kept.apply(&OfflineSettings::set_policy(Some(OfflinePolicy::Required)));
    store.hold(Some(offline), kept);
    editor.hold(None, SettingsContent::default());
    editor.run();

    assert!(
        editor
            .document()
            .find_test_id(&format!("settings.offline.{team}"))
            .is_some(),
        "each workspace can be kept offline"
    );
    editor.snapshot(
        "an_organizations_policies_show_to_its_administrators_and_fix_what_devices_keep_offline",
    );

    editor.click("settings.organization.workspaces");
    editor.run();
    editor.key(Key::ArrowDown, Modifiers::NONE);
    editor.run();
    editor.key(Key::Enter, Modifiers::NONE);
    editor.run();
    assert_eq!(
        editor
            .content::<OrganizationPoliciesContent>(Some(policies))
            .root()
            .workspace_creation(),
        WhoMay::Administrators
    );
}
