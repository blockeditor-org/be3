use super::*;

/// The organization's settings, held at the root before the mirror knew whose they are, sat in
/// the root's listing as any block there does. Once it knows, they leave it at once, not at the
/// root's next answer: once nothing else holds them they leave the mirror, and that answer,
/// which leaves them out, loses nothing.
#[test]
fn a_root_listing_made_before_the_organization_was_known_lets_its_settings_go() {
    let organization = Uuid::from_u128(7);
    let settings = be_block::organization_settings(organization);
    let roots = Listing::Children(BlockParent::Root);
    let mut graph = Graph::default();
    graph.want(Owner::Host, vec![Query::Roots, Query::Block(settings)]);
    assert!(!graph.take_wanted().is_empty());
    graph.listed(Listing::Block(settings), vec![node(settings)], 1);

    graph.organized(organization);
    assert!(
        graph.query(Query::Roots).is_empty(),
        "the organization's settings are no root block"
    );
    assert!(graph.get(settings).is_some(), "held while asked for");
    graph.want(Owner::Host, vec![Query::Roots]);
    assert!(
        graph.get(settings).is_none(),
        "the root's listing kept the settings once nothing else wanted them"
    );

    graph.listed(roots, Vec::new(), 2);
    assert_eq!(
        graph.take_unlisted_held(),
        [],
        "the root's answer, which leaves the settings out, loses nothing"
    );
}
