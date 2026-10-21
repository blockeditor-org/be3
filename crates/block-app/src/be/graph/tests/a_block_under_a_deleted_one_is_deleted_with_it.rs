use super::*;

#[test]
fn a_block_under_a_deleted_one_is_deleted_with_it() {
    let folder = Uuid::from_u128(1);
    let inside = Uuid::from_u128(2);
    let kept = Uuid::from_u128(3);
    let shared = Uuid::from_u128(4);
    let mut graph = Graph::default();
    listed(
        &mut graph,
        vec![
            Node {
                parent: Some(BlockParent::Detached),
                ..node(folder)
            },
            Node {
                parent: Some(BlockParent::Block(folder)),
                ..node(inside)
            },
            node(kept),
            Node {
                parent: Some(BlockParent::Block(Uuid::from_u128(99))),
                ..node(shared)
            },
        ],
        1,
    );

    assert_eq!(graph.deleted_root(folder), Some(folder));
    assert_eq!(graph.deleted_root(inside), Some(folder));
    assert_eq!(graph.deleted_root(kept), None);
    assert_eq!(
        graph.deleted_root(shared),
        None,
        "a block whose parent it cannot see is not taken for deleted"
    );

    graph.placing(folder, true);
    assert_eq!(
        graph.deleted_root(inside),
        None,
        "a block the host is placing is not taken for deleted while it waits"
    );
    graph.placing(folder, false);
    assert_eq!(graph.deleted_root(inside), Some(folder));

    graph.update(folder, |node| node.parent = Some(BlockParent::Root));
    assert_eq!(
        graph.deleted_root(inside),
        None,
        "restoring the folder restores what is inside"
    );
}
