use super::*;

#[test]
fn an_unfiled_holds_its_children_by_parent_alone() {
    let mut graph = BlockGraph::new();
    let mut unfiled = BlockNode::new(TYPE, AUTHOR, BlockParent::Root);
    unfiled.unfiled = true;
    graph.insert(id(1), unfiled).unwrap();
    graph
        .insert(
            id(2),
            BlockNode::new(TYPE, AUTHOR, BlockParent::Block(id(1))),
        )
        .unwrap();
    graph
        .insert(
            id(3),
            BlockNode::new(TYPE, AUTHOR, BlockParent::Block(id(2))),
        )
        .unwrap();
    assert!(graph.is_live(id(2)), "Unfiled lists nothing, yet holds it");
    assert!(
        !graph.is_live(id(3)),
        "a block below one in Unfiled still needs its parent to list it"
    );
    graph.set_references(id(2), &[id(3)]).unwrap();
    assert!(graph.is_live(id(3)));
    assert!(graph.deleted_roots().is_empty());

    graph.set_parent(id(2), BlockParent::Detached).unwrap();
    assert!(!graph.is_live(id(2)));
    assert!(!graph.is_live(id(3)), "it goes with the block above it");
    assert_eq!(graph.deleted_roots(), vec![id(2)]);

    graph.set_parent(id(2), BlockParent::Block(id(1))).unwrap();
    assert!(
        graph.is_live(id(3)),
        "restoring it to Unfiled brings it back"
    );
    assert!(graph.deleted_roots().is_empty());
}
