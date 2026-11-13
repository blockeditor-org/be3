use super::*;

/// Places `child` in `files` as the host does: placing, its parent set, then the parent's
/// replica reporting it among its references.
fn place(graph: &mut Graph, files: Uuid, child: Uuid) {
    graph.placing(child, true);
    graph.change(Node {
        parent: Some(BlockParent::Block(files)),
        ..node(child)
    });
    graph.refer(files, vec![child]);
    assert!(graph.take_dropped().is_empty());
    assert_eq!(graph.deleted_root(child), None, "placed, it is held");
}

#[test]
fn a_placed_child_its_parent_drops_again_is_noted() {
    let (files, child, elsewhere) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
    let mut graph = Graph::default();
    listed(&mut graph, vec![node(files), node(elsewhere)], 1);

    // The parent's replica reports references without it: noted, and it reads as deleted.
    place(&mut graph, files, child);
    graph.refer(files, Vec::new());
    assert_eq!(
        graph.take_dropped(),
        [Dropped {
            child,
            parent: files,
            by: "its open replica's report",
        }]
    );
    assert_eq!(graph.deleted_root(child), Some(child));

    // A listing from before it was placed, once the replica no longer reports: noted as such.
    place(&mut graph, files, child);
    graph.forget_references(files);
    graph.listed(Listing::Block(files), vec![node(files)], 2);
    assert_eq!(
        graph.take_dropped(),
        [Dropped {
            child,
            parent: files,
            by: "a listing",
        }]
    );

    // A child moved elsewhere is not one its old parent dropped.
    place(&mut graph, files, child);
    graph.update(child, |node| {
        node.parent = Some(BlockParent::Block(elsewhere))
    });
    graph.refer(files, Vec::new());
    assert!(graph.take_dropped().is_empty());
}
