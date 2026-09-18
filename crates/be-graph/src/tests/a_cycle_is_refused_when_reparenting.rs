use super::*;

#[test]
fn a_cycle_is_refused_when_reparenting() {
    let mut graph = graph(&[
        (1, BlockParent::Root),
        (2, BlockParent::Block(id(1))),
        (3, BlockParent::Block(id(2))),
    ]);

    assert_eq!(
        graph.set_parent(id(1), BlockParent::Block(id(3))),
        Err(GraphError::ParentCycle)
    );
    assert_eq!(
        graph.set_parent(id(1), BlockParent::Block(id(1))),
        Err(GraphError::ParentCycle)
    );
    assert_eq!(
        graph.set_parent(id(9), BlockParent::Root),
        Err(GraphError::NotFound(id(9)))
    );
    assert_eq!(
        graph.insert(id(1), BlockNode::new(TYPE, AUTHOR, BlockParent::Root)),
        Err(GraphError::AlreadyExists(id(1)))
    );

    graph.set_parent(id(3), BlockParent::Root).unwrap();
    graph.set_parent(id(1), BlockParent::Block(id(3))).unwrap();
    assert!(graph.is_live(id(2)));
}
