use block_editor_beui::be_block::{
    Home, HomeContent, PrefixRegistry, PrefixRegistryContent, WORKSPACE_TYPE,
};

use super::*;
use crate::home::HomeApp;
use crate::prefixes::PrefixesApp;

fn named(id: Uuid, name: &str, parent: Uuid) -> BlockInfo {
    let mut info = BlockInfo::new(id, Uuid::from_u128(1), BlockParent::Block(parent));
    info.set_name(name);
    info
}

#[test]
fn a_workspaces_home_and_prefixes_list_what_they_hold() {
    let (workspace, kind) = (Uuid::new_v4(), Uuid::new_v4());
    let (issues, bugs, milestones) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let host = EditorHost::default();
    host.set_block_type(PrefixRegistryContent::CONTENT_TYPE);
    host.set_editable(true);
    let mut prefixes = BeuiTest::<PrefixesApp>::new(OwnedEditor::new(host, kind));
    let store = prefixes.store();
    store.add_block(BlockInfo::new(workspace, WORKSPACE_TYPE, BlockParent::Root));
    for (id, name) in [(issues, "Issues"), (bugs, "Bugs"), (milestones, "Milestones")] {
        store.add_block(named(id, name, workspace));
    }
    let mut listed = PrefixRegistryContent::default();
    listed.apply(&PrefixRegistry::register(issues, Some("BE")));
    listed.apply(&PrefixRegistry::register(bugs, Some("be")));
    listed.apply(&PrefixRegistry::register(milestones, Some("MS")));
    prefixes.hold(None, listed);
    prefixes.run();
    prefixes.snapshot("a_workspaces_prefixes_say_which_one_two_databases_share");

    let (home, plan, guide) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let host = EditorHost::default();
    host.set_block_type(HomeContent::CONTENT_TYPE);
    host.set_editable(true);
    let mut shown = BeuiTest::<HomeApp>::new(OwnedEditor::new(host, home));
    let store = shown.store();
    store.add_block(BlockInfo::new(workspace, WORKSPACE_TYPE, BlockParent::Root));
    store.add_block(named(plan, "Plan", workspace));
    store.add_block(named(guide, "Guide", workspace));
    let mut kept = HomeContent::default();
    kept.apply(&Home::set_landing(Some(guide)));
    kept.apply(&Home::pin(plan, true));
    kept.apply(&Home::keep_template(plan, true));
    shown.hold(None, kept);
    shown.run();
    shown.snapshot("a_workspaces_home_lists_its_landing_page_pins_and_templates");
    shown.click(format!("home.{guide}.remove"));
    shown.run();
    assert_eq!(
        shown.content::<HomeContent>(None).root().landing,
        None,
        "removing the landing page leaves the workspace without one"
    );
}
