use super::*;

use be_block::SettingsProfileContent;

#[test]
fn an_owner_only_kind_makes_its_block_owner_only() {
    let graph = Graph {
        loaded: true,
        ..Graph::default()
    };
    let kind = RootSetting::<SettingsProfileContent>::owner_only()
        .ensure(&graph, Uuid::nil())
        .unwrap();
    let ordinary = RootSetting::<SettingsProfileContent>::default()
        .ensure(&graph, Uuid::nil())
        .unwrap();
    let created = graph.created.borrow().clone();
    let owner_only = |block: Uuid| {
        created
            .iter()
            .find(|(id, _, _)| *id == block)
            .map(|(_, _, owner_only)| *owner_only)
    };
    assert_eq!(owner_only(kind), Some(true));
    assert_eq!(owner_only(ordinary), Some(false));
    let settings = graph.settings.borrow().unwrap();
    assert_eq!(
        owner_only(settings),
        Some(false),
        "the account's Settings block stays readable by administrators"
    );
}
