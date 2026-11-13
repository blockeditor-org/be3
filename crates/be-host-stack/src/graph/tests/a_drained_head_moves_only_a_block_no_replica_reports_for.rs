use super::*;

fn commit(name: &[u8]) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::of(name))
}

#[test]
fn a_drained_head_moves_only_a_block_no_replica_reports_for() {
    let block = Uuid::from_u128(1);
    let (h1, h2, h3) = (commit(b"h1"), commit(b"h2"), commit(b"h3"));
    let mut graph = Graph::default();
    listed(&mut graph, vec![node(block)], 1);

    // An open block's replica reported a newer head: a late drain of an older one leaves it.
    graph.report_head(block, Some(h2));
    graph.drained_head(block, h1);
    assert_eq!(graph.get(block).and_then(|node| node.head), Some(h2));

    // Once the block is closed, the drain is its head.
    graph.forget_reported_head(block);
    graph.drained_head(block, h3);
    assert_eq!(graph.get(block).and_then(|node| node.head), Some(h3));
}
