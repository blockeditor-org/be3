use super::*;

fn commit(name: &[u8]) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::of(name))
}

/// No summary names what a block references: the mirror reads it from the block's head once it
/// shows what the block holds, and until then a query of its references waits, and its children
/// are held as the server says. Content that cannot be read answers what the server lists, and
/// at a new head what was read before shows until the new head is read.
#[test]
fn a_blocks_references_are_read_from_its_head_not_its_summary() {
    let folder = Uuid::from_u128(1);
    let (first, second) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let (one, two) = (commit(b"one"), commit(b"two"));
    let at = |head| Node {
        head: Some(head),
        ..node(folder)
    };
    let child = |id| Node {
        parent: Some(BlockParent::Block(folder)),
        ..node(id)
    };
    let ids = |nodes: Vec<Node>| nodes.into_iter().map(|node| node.id).collect::<Vec<_>>();

    let mut graph = Graph::default();
    graph.want(Owner::Editor(1), vec![Query::References(folder)]);
    graph.take_wanted();
    graph.listed(Listing::Block(folder), vec![at(one)], 1);
    graph.listed(
        Listing::References(folder),
        vec![child(first), child(second)],
        1,
    );
    assert_eq!(graph.get(folder).and_then(|node| node.references.clone()), None);
    assert!(
        !graph.ready(Query::References(folder)),
        "its order waits for its content"
    );
    assert_eq!(graph.deleted_root(first), None, "held as the server says");
    assert_eq!(graph.take_to_read(), [(folder, one)]);
    assert!(graph.take_to_read().is_empty(), "read once at a time");

    // A failure that may pass (a fetch cut off) is tried again once the server next answers,
    // and pinned after `READ_ATTEMPTS`.
    for _ in 1..READ_ATTEMPTS {
        graph.read_failed(folder, one, false);
        assert!(graph.take_to_read().is_empty(), "waits for the server's next answer");
        graph.listed(
            Listing::References(folder),
            vec![child(first), child(second)],
            1,
        );
        assert_eq!(graph.take_to_read(), [(folder, one)], "tried again");
    }

    // Its content cannot be read for a reason that lasts (no key opens it, or the attempts ran
    // out): the server's listing answers, in the order of ids, and it is not tried again on this
    // connection.
    graph.read_failed(folder, one, false);
    assert!(graph.ready(Query::References(folder)));
    assert_eq!(ids(graph.query(Query::References(folder))), [first, second]);
    assert!(graph.take_to_read().is_empty());
    graph.relist_all();
    assert_eq!(graph.take_to_read(), [(folder, one)], "a new connection tries again");
    graph.take_wanted();
    graph.listed(Listing::Block(folder), vec![at(one)], 1);
    graph.listed(
        Listing::References(folder),
        vec![child(first), child(second)],
        1,
    );

    graph.read_references(folder, one, vec![second, first]);
    assert!(graph.ready(Query::References(folder)));
    assert_eq!(ids(graph.query(Query::References(folder))), [second, first]);
    assert_eq!(ids(graph.query(Query::Backrefs(first))), [folder]);

    // Listed again at the same head: what was read stays.
    graph.listed(Listing::Block(folder), vec![changed_at(at(one), 2)], 2);
    assert_eq!(ids(graph.query(Query::References(folder))), [second, first]);
    assert!(graph.take_to_read().is_empty());

    // At a new head, what was read before shows until the new head is read.
    graph.listed(Listing::Block(folder), vec![changed_at(at(two), 3)], 3);
    assert!(graph.ready(Query::References(folder)));
    assert_eq!(ids(graph.query(Query::References(folder))), [second, first]);
    assert_eq!(graph.take_to_read(), [(folder, two)]);
    graph.read_references(folder, two, vec![first]);
    assert_eq!(ids(graph.query(Query::References(folder))), [first]);
    assert_eq!(
        graph.deleted_root(second),
        Some(second),
        "a child its parent's content no longer holds is deleted"
    );
}
