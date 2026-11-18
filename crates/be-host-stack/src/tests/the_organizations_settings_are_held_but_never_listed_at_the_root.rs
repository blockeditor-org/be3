use super::*;

/// The organization's settings lie at the root, but the server's listing of the root leaves
/// them out: held for what reads them, they are neither a root block nor a member of the root's
/// listing, so its answer is not taken for one that lost them.
#[test]
fn the_organizations_settings_are_held_but_never_listed_at_the_root() {
    let harness = Harness::start();
    harness.connect();
    let settings = be_block::organization_settings(harness.organization);
    ask_for(vec![Query::Block(settings)]);
    wait_until("held the organization's settings", |shared| {
        shared.graph.ready(Query::Block(settings))
            && shared
                .graph
                .get(settings)
                .is_some_and(|node| node.parent == Some(be_block_graph::BlockParent::Root))
    });

    ask_for(vec![Query::Roots]);
    wait_until("listed the roots", |shared| shared.graph.ready(Query::Roots));
    let unlisted = crate::wait_until("read what the answer did", |shared| {
        Some(
            shared
                .unlisted_logged
                .iter()
                .chain(shared.graph.unlisted_held())
                .cloned()
                .collect::<Vec<_>>(),
        )
    });
    assert!(
        !query(Query::Roots).iter().any(|node| node.id == settings),
        "the organization's settings are no root block"
    );
    assert_eq!(
        unlisted,
        [],
        "the root's listing never held the organization's settings, so leaving them out loses \
         nothing"
    );
}
