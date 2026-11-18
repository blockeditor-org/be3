use super::*;

fn commit(name: &[u8]) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::of(name))
}

/// A block counts among another's backrefs by what it references at its head. At a new head the
/// references read at the last one still show, but say nothing of what it points at now: until
/// the new head is read, the server's listing says, and an answer that leaves the block out
/// loses nothing this device knows.
#[test]
fn a_backref_counts_by_the_references_read_at_its_head() {
    let (folder, target) = (Uuid::from_u128(1), Uuid::from_u128(2));
    let (one, two) = (commit(b"one"), commit(b"two"));
    let at = |head, changed| Node {
        head: Some(head),
        changed,
        ..node(folder)
    };
    let backrefs = Listing::Backrefs(target);
    let ids = |nodes: Vec<Node>| nodes.into_iter().map(|node| node.id).collect::<Vec<_>>();

    let mut graph = Graph::default();
    graph.want(
        Owner::Editor(1),
        vec![Query::Block(folder), Query::Backrefs(target)],
    );
    assert!(!graph.take_wanted().is_empty());
    graph.listed(Listing::Block(folder), vec![at(one, 1)], 1);
    graph.read_references(folder, one, vec![target]);
    graph.listed(backrefs, vec![at(one, 1)], 1);
    assert_eq!(
        ids(graph.query(Query::Backrefs(target))),
        [folder],
        "read at its head, the folder points at the block"
    );

    // Its new head no longer points at the block, and is not read yet.
    graph.listed(Listing::Block(folder), vec![at(two, 2)], 2);
    assert_eq!(
        graph.get(folder).and_then(|node| node.references.clone()),
        Some(vec![target]),
        "what was read at the last head shows meanwhile"
    );
    graph.relist(backrefs);
    assert!(!graph.take_wanted().is_empty());
    graph.listed(backrefs, Vec::new(), 2);
    assert_eq!(
        ids(graph.query(Query::Backrefs(target))),
        Vec::<Uuid>::new(),
        "references read at an older head kept the folder among the block's backrefs"
    );
    assert_eq!(
        graph.take_unlisted_held(),
        [],
        "the answer left out nothing this device knows at the folder's head"
    );
}
