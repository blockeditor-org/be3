use super::*;

#[test]
fn a_detached_subtree_is_pending_deletion() {
    let mut graph = graph(&[
        (1, BlockParent::Root),
        (2, BlockParent::Block(id(1))),
        (3, BlockParent::Block(id(2))),
        (4, BlockParent::Block(id(1))),
    ]);
    assert!(graph.ids().iter().all(|block| graph.is_live(*block)));
    assert!(graph.detached().is_empty());

    graph.set_parent(id(2), BlockParent::Detached).unwrap();

    assert!(graph.is_live(id(1)));
    assert!(graph.is_live(id(4)));
    assert!(!graph.is_live(id(2)));
    assert!(!graph.is_live(id(3)));
    assert_eq!(graph.detached(), vec![id(2), id(3)]);
    assert_eq!(graph.subtree(id(2)), vec![id(2), id(3)]);

    for block in graph.subtree(id(2)) {
        graph.remove(block);
    }
    assert_eq!(graph.ids(), vec![id(1), id(4)]);
    assert!(graph.detached().is_empty());
}
