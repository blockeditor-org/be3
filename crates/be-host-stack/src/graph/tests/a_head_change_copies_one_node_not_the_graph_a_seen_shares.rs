use super::*;

use std::sync::atomic::Ordering;

fn commit(name: &[u8]) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::of(name))
}

/// The worker hands the page a `Seen` that shares the graph (`seen::Part`), and changes its own
/// right after: a replica reports its head on every edit, and other devices' heads arrive one
/// by one. Each change copies the one node it changes, never the graph, however large it is,
/// and a report that changes nothing copies nothing.
#[test]
fn a_head_change_copies_one_node_not_the_graph_a_seen_shares() {
    let mut graph = Graph::default();
    graph.want(Owner::Host, vec![Query::Roots]);
    let nodes: Vec<Node> = (1..=20_000).map(|id| node(Uuid::from_u128(id))).collect();
    graph.listed(Listing::Children(BlockParent::Root), nodes, 1);
    let (typed, remote) = (Uuid::from_u128(7), Uuid::from_u128(19_000));
    let handed = graph.clone();

    NODES_COPIED.store(0, Ordering::Relaxed);
    graph.report_head(typed, Some(commit(b"sealed")));
    assert_eq!(
        NODES_COPIED.load(Ordering::Relaxed),
        1,
        "the replica's own seal copies its node"
    );

    NODES_COPIED.store(0, Ordering::Relaxed);
    let handed_again = graph.clone();
    assert!(
        graph.reports(typed, Some(commit(b"sealed"))),
        "a report of the same head changes nothing"
    );
    assert_eq!(NODES_COPIED.load(Ordering::Relaxed), 0);

    graph.set_head(remote, Some(commit(b"another device's")));
    assert_eq!(
        NODES_COPIED.load(Ordering::Relaxed),
        1,
        "a head another device published copies its node"
    );

    assert_eq!(handed.get(typed).and_then(|node| node.head), None);
    assert_eq!(handed_again.get(remote).and_then(|node| node.head), None);
    assert_eq!(
        graph.get(typed).and_then(|node| node.head),
        Some(commit(b"sealed"))
    );
    assert_eq!(
        graph.get(remote).and_then(|node| node.head),
        Some(commit(b"another device's"))
    );
}
