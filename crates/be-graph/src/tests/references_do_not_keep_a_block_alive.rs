use super::*;

#[test]
fn references_do_not_keep_a_block_alive() {
    let mut graph = graph(&[
        (1, BlockParent::Root),
        (2, BlockParent::Block(id(1))),
        (3, BlockParent::Root),
    ]);
    graph.apply_references(id(3), &[id(2)], &[]).unwrap();
    assert_eq!(graph.references(id(3)), vec![id(2)]);
    assert_eq!(graph.backrefs(id(2)), vec![id(3)]);

    graph.set_parent(id(2), BlockParent::Detached).unwrap();
    assert!(
        !graph.is_live(id(2)),
        "a backref kept a detached block alive"
    );
    assert_eq!(graph.backrefs(id(2)), vec![id(3)]);

    graph.remove(id(2));
    assert!(graph.references(id(3)).is_empty());
    assert!(graph.backrefs(id(2)).is_empty());

    graph.apply_references(id(3), &[id(1)], &[]).unwrap();
    graph.apply_references(id(3), &[], &[id(1)]).unwrap();
    assert!(graph.references(id(3)).is_empty());
    assert!(graph.backrefs(id(1)).is_empty());
}
