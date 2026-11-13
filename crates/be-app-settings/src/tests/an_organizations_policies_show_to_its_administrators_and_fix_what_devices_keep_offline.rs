use be_sdk_beui::BeuiRoot;
use be_sdk_beui::be_block::{OfflinePolicy, OfflineSettings, OfflineSettingsContent, OrganizationPolicies, OrganizationPoliciesContent, WORKSPACE_TYPE, WhoMay, organization_settings};
use be_sdk_test::ContentStore;

use super::*;
use crate::root::{OfflineRoot, OrganizationRoot};

fn named(id: Uuid, block_type: Uuid, parent: BlockParent, name: &str) -> BlockInfo {
    let mut info = BlockInfo::new(id, block_type, parent);
    info.set_name(name);
    info
}

struct Organization {
    own: Uuid,
    settings: Uuid,
    policies: Uuid,
    offline: Uuid,
    team: Uuid,
    notes: Uuid,
    kept_here: Uuid,
}

impl Organization {
    fn new() -> Self {
        let settings = organization_settings(Uuid::from_u128(0x0e0f));
        let [own, policies, offline, team, notes, kept_here] =
            [1, 2, 3, 4, 5, 6].map(|id| Uuid::from_u128(0x0e00 + id));
        Self {
            own,
            settings,
            policies,
            offline,
            team,
            notes,
            kept_here,
        }
    }

    fn open<R: BeuiRoot>(&self, block: Uuid, size: Vec2) -> BeuiTest<R> {
        let host = EditorHost::default();
        host.set_account_id(Uuid::nil());
        host.set_organization_id(Uuid::from_u128(0x0e0f));
        host.set_editable(true);
        host.set_client_id(Uuid::from_u128(0x0e10));
        let mut editor = BeuiTest::<R>::new(OwnedEditor::new(host, block));
        editor.set_size(size);
        self.fill(&editor.store());
        editor.run();
        editor
    }

    fn fill(&self, store: &ContentStore) {
        store.add_block(BlockInfo::new(
            self.own,
            SettingsContent::CONTENT_TYPE,
            BlockParent::Root,
        ));
        store.add_block(BlockInfo::new(
            self.kept_here,
            OfflineSettingsContent::CONTENT_TYPE,
            BlockParent::Block(self.own),
        ));
        store.add_block(named(self.team, WORKSPACE_TYPE, BlockParent::Root, "Team"));
        store.add_block(named(
            self.notes,
            WORKSPACE_TYPE,
            BlockParent::Root,
            "Notes",
        ));
        store.add_block(BlockInfo::new(
            self.settings,
            SettingsContent::CONTENT_TYPE,
            BlockParent::Root,
        ));
        for (id, block_type) in [
            (self.policies, OrganizationPoliciesContent::CONTENT_TYPE),
            (self.offline, OfflineSettingsContent::CONTENT_TYPE),
        ] {
            store.add_block(BlockInfo::new(
                id,
                block_type,
                BlockParent::Block(self.settings),
            ));
        }
        let mut own = SettingsContent::default();
        own.apply(&Settings::set_kind(
            OfflineSettingsContent::CONTENT_TYPE,
            self.kept_here,
        ));
        store.hold(Some(self.own), own);
        store.hold(Some(self.kept_here), OfflineSettingsContent::default());
        let mut held = SettingsContent::default();
        held.apply(&Settings::set_policy(
            OrganizationPoliciesContent::CONTENT_TYPE,
            self.policies,
        ));
        held.apply(&Settings::set_policy(
            OfflineSettingsContent::CONTENT_TYPE,
            self.offline,
        ));
        store.hold(Some(self.settings), held);
        let mut set = OrganizationPoliciesContent::default();
        set.apply(&OrganizationPolicies::set_history_days(Some(30)));
        store.hold(Some(self.policies), set);
        let mut kept = OfflineSettingsContent::default();
        kept.apply(&OfflineSettings::set_policy(Some(OfflinePolicy::Required)));
        store.hold(Some(self.offline), kept);
    }
}

#[test]
fn an_organizations_policies_show_to_its_administrators_and_fix_what_devices_keep_offline() {
    let organization = Organization::new();

    let mut offline =
        organization.open::<OfflineRoot>(organization.kept_here, Vec2::new(720.0, 360.0));
    for workspace in [organization.team, organization.notes] {
        assert!(
            offline
                .document()
                .find_test_id(&format!("settings.offline.{workspace}"))
                .is_some(),
            "each workspace can be kept offline"
        );
    }
    offline.snapshot("offline_follows_what_the_organization_requires");

    let mut editor =
        organization.open::<OrganizationRoot>(organization.settings, Vec2::new(720.0, 1000.0));
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
            .content::<OrganizationPoliciesContent>(Some(organization.policies))
            .root()
            .workspace_creation(),
        WhoMay::Administrators
    );
}
