use super::*;

#[test]
fn a_block_placed_in_another_is_listed_as_its_reference_before_the_server_hears_of_it() {
    let canvas = Uuid::from_u128(1);
    let placed = Uuid::from_u128(2);
    let mut graph = Graph::default();
    graph.want(Owner::Editor(1), vec![Query::References(canvas)]);
    graph.take_wanted();
    graph.listed(Listing::References(canvas), Vec::new(), 1);
    graph.listed(Listing::Block(canvas), vec![node(canvas)], 1);

    graph.placing(placed, true);
    graph.change(Node {
        parent: Some(BlockParent::Block(canvas)),
        ..node(placed)
    });
    graph.settle(placed);
    graph.put(
        Node {
            parent: Some(BlockParent::Block(canvas)),
            ..node(placed)
        },
        2,
    );
    assert!(
        graph.get(placed).is_some(),
        "a block being placed is kept after the server takes it, before its parent refers to it"
    );

    graph.refer(canvas, vec![placed]);
    graph.take_wanted();
    graph.listed(Listing::References(canvas), Vec::new(), 2);
    assert!(graph.ready(Query::References(canvas)));
    assert_eq!(
        graph
            .query(Query::References(canvas))
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>(),
        [placed],
        "the parent's own description lists the block it refers to while the server's listing \
         is from before the parent's edit reached it"
    );
}
