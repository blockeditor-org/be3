use be_block::be_model::BlockRef;

use super::*;

#[test]
fn a_block_s_children_leave_out_what_its_metadata_points_to() {
    let canvas = Uuid::from_u128(1);
    let child = Uuid::from_u128(2);
    let tag = Uuid::from_u128(3);
    let settings = Uuid::from_u128(4);
    let mut metadata = BlockMetadata::default();
    metadata.insert(&be_block::TAGGED, &vec![BlockRef(tag)]);
    metadata.insert(&be_block::SETTINGS, &BlockRef(settings));
    let mut graph = Graph::default();
    listed(
        &mut graph,
        vec![
            Node {
                references: vec![child, tag, settings],
                metadata,
                ..node(canvas)
            },
            Node {
                parent: Some(BlockParent::Block(canvas)),
                ..node(child)
            },
            node(tag),
            Node {
                parent: Some(BlockParent::Block(canvas)),
                ..node(settings)
            },
        ],
        1,
    );
    let ids = |nodes: Vec<Node>| nodes.into_iter().map(|node| node.id).collect::<Vec<_>>();

    assert_eq!(
        ids(graph.query(Query::References(canvas))),
        vec![child],
        "a block's children are what its content refers to"
    );
    assert_eq!(ids(graph.query(Query::Backrefs(tag))), vec![canvas]);
    assert_eq!(graph.deleted_root(settings), None);

    graph.refer(canvas, vec![child]);
    assert_eq!(
        ids(graph.query(Query::Backrefs(tag))),
        vec![canvas],
        "an open replica's references leave its metadata's in place"
    );
    assert_eq!(
        graph.deleted_root(settings),
        None,
        "and its settings stay held"
    );
    assert_eq!(ids(graph.query(Query::References(canvas))), vec![child]);
}
