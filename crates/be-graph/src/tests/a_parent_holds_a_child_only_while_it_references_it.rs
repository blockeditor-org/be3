use super::*;

#[test]
fn a_parent_holds_a_child_only_while_it_references_it() {
    let mut graph = graph(&[
        (1, BlockParent::Root),
        (2, BlockParent::Block(id(1))),
        (3, BlockParent::Block(id(2))),
        (4, BlockParent::Block(id(1))),
    ]);
    assert!(graph.ids().iter().all(|block| graph.is_live(*block)));
    assert!(graph.deleted_roots().is_empty());

    graph.set_references(id(1), &[id(4)]).unwrap();
    assert_eq!(graph.get(id(2)).unwrap().parent, BlockParent::Block(id(1)));
    assert!(!graph.is_live(id(2)), "its parent no longer lists it");
    assert!(!graph.is_live(id(3)), "it goes with the block above it");
    assert_eq!(graph.deleted_root(id(3)), Some(id(2)));
    assert_eq!(graph.deleted_roots(), vec![id(2)]);
    assert_eq!(graph.detached(), vec![id(2), id(3)]);

    graph.set_references(id(1), &[id(4), id(2)]).unwrap();
    assert!(graph.is_live(id(3)), "listing it again brings it back");
    assert!(graph.deleted_roots().is_empty());

    graph.set_parent(id(2), BlockParent::Detached).unwrap();
    assert!(!graph.is_live(id(2)), "a reference alone holds nothing");
    assert_eq!(graph.deleted_roots(), vec![id(2)]);

    graph.remove(id(4));
    graph
        .insert(id(5), BlockNode::new(TYPE, AUTHOR, BlockParent::Root))
        .unwrap();
    graph
        .insert(
            id(6),
            BlockNode::new(TYPE, AUTHOR, BlockParent::Block(id(5))),
        )
        .unwrap();
    assert!(
        !graph.is_live(id(6)),
        "a new child waits for its parent to list it"
    );
    graph.set_references(id(5), &[id(6)]).unwrap();
    assert!(graph.is_live(id(6)));
}
