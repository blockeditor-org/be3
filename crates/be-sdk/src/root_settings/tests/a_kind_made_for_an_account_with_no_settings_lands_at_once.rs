use super::*;

use be_block::SettingsProfileContent;

/// The account's `Settings` and the kind are made together, so the kind is listed in the
/// `Settings` from its first version: nothing waits on reading back a block just made.
#[test]
fn a_kind_made_for_an_account_with_no_settings_lands_at_once() {
    let graph = Graph::default();

    let kind = RootSetting::<SettingsProfileContent>::default()
        .ensure(&graph, Uuid::nil())
        .expect("the kind is made on the first try");

    let settings = graph
        .settings
        .borrow()
        .expect("the account's Settings is made");
    let contents = graph.contents.borrow();
    let made = contents
        .iter()
        .find(|(block, _)| *block == settings)
        .map(|(_, bytes)| SettingsContent::decode(bytes).expect("Settings content"))
        .expect("the Settings block was made with content");
    assert_eq!(
        made.root().kind(SettingsProfileContent::CONTENT_TYPE),
        Some(kind)
    );
    assert!(contents.iter().any(|(block, _)| *block == kind));
}
