use super::*;

#[test]
fn a_blocks_references_listed_before_the_block_itself_are_kept() {
    let view = Uuid::from_u128(1);
    let linked = Uuid::from_u128(2);
    let mut graph = Graph::default();
    graph.want(Owner::Editor(1), vec![Query::References(view)]);
    graph.take_wanted();

    graph.listed(Listing::References(view), vec![node(linked)], 1);
    assert!(
        graph.get(linked).is_some(),
        "the server's answer is kept while the block that links to it is still unknown"
    );

    let linking = Node {
        references: vec![linked],
        ..node(view)
    };
    graph.listed(Listing::Block(view), vec![linking], 1);
    assert!(graph.ready(Query::References(view)));
    assert_eq!(
        graph
            .query(Query::References(view))
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>(),
        [linked]
    );
}
