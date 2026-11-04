use super::*;

#[test]
fn a_tree_size_follows_its_blocks_as_they_move() {
    let mut graph = graph(&[
        (1, BlockParent::Root),
        (2, BlockParent::Block(id(1))),
        (3, BlockParent::Block(id(2))),
        (4, BlockParent::Root),
    ]);
    let sizes = |graph: &BlockGraph, block: u128| {
        let node = graph.get(id(block)).unwrap();
        (node.size, node.tree_size)
    };

    graph.add_size(id(3), 100, true).unwrap();
    graph.add_size(id(2), 10, true).unwrap();
    assert_eq!(sizes(&graph, 1), (0, 110));
    assert_eq!(sizes(&graph, 2), (10, 110));
    assert_eq!(sizes(&graph, 3), (100, 100));

    graph.set_parent(id(2), BlockParent::Block(id(4))).unwrap();
    assert_eq!(
        sizes(&graph, 1),
        (0, 0),
        "a moved subtree left its old parent"
    );
    assert_eq!(
        sizes(&graph, 4),
        (0, 110),
        "a moved subtree joined its new parent"
    );

    graph.add_size(id(3), 40, false).unwrap();
    assert_eq!(sizes(&graph, 4), (0, 70));

    graph.remove(id(2));
    assert_eq!(
        sizes(&graph, 4),
        (0, 0),
        "a removed block took its subtree's bytes"
    );
    assert_eq!(
        sizes(&graph, 3),
        (60, 60),
        "an orphaned child keeps its own tree size"
    );
}
