use super::*;

fn commit(name: &[u8]) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::of(name))
}

/// Another device adds a child to a folder: the folder's new head is not read yet, so what it
/// held at the last head read shows, and the child the server says it holds is not taken for
/// deleted meanwhile. Once the new head is read, its content decides.
#[test]
fn a_child_added_at_a_head_not_read_yet_is_not_taken_for_deleted() {
    let folder = Uuid::from_u128(1);
    let (first, added) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let (one, two) = (commit(b"one"), commit(b"two"));
    let at = |head| Node {
        head: Some(head),
        ..node(folder)
    };
    let child = |id, held| Node {
        parent: Some(BlockParent::Block(folder)),
        held,
        ..node(id)
    };
    let mut graph = Graph::default();
    graph.want(Owner::Editor(1), vec![Query::Children(folder)]);
    graph.take_wanted();
    graph.listed(Listing::Block(folder), vec![at(one)], 1);
    graph.listed(
        Listing::Children(BlockParent::Block(folder)),
        vec![child(first, true)],
        1,
    );
    assert_eq!(graph.take_to_read(), [(folder, one)]);
    graph.read_references(folder, one, vec![first]);

    graph.listed(Listing::Block(folder), vec![changed_at(at(two), 2)], 2);
    graph.put(changed_at(child(added, true), 2), 2);
    assert_eq!(
        graph.deleted_root(added),
        None,
        "held as the server says until the folder's new head is read"
    );
    assert_eq!(graph.take_to_read(), [(folder, two)]);
    graph.read_references(folder, two, vec![first, added]);
    assert_eq!(graph.deleted_root(added), None);

    // One neither the content read nor the server holds is deleted.
    let gone = Uuid::from_u128(4);
    graph.put(changed_at(child(gone, false), 3), 3);
    assert_eq!(graph.deleted_root(gone), Some(gone));
}
