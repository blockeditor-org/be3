use super::*;

fn commit(name: &[u8]) -> CommitId {
    CommitId::from_hash(be_block_store::Hash::of(name))
}

#[test]
fn a_head_an_open_block_reported_outlasts_what_is_listed_of_it() {
    let block = Uuid::from_u128(1);
    let (old, reported) = (commit(b"old"), commit(b"reported"));
    let mut graph = Graph::default();

    // Its replica reports a commit before the graph has the block: the head is kept for it.
    graph.report_head(block, Some(reported));
    listed(
        &mut graph,
        vec![Node {
            head: Some(old),
            ..node(block)
        }],
        1,
    );
    assert_eq!(graph.get(block).and_then(|node| node.head), Some(reported));

    // What the server lists of it afterwards, made before the commit reached it, does not undo it.
    graph.listed(
        Listing::Block(block),
        vec![Node {
            head: Some(old),
            ..node(block)
        }],
        2,
    );
    assert_eq!(graph.get(block).and_then(|node| node.head), Some(reported));

    // A head another writer sets is the one a later listing leaves alone, not the one before it.
    let (merged, listed_before) = (commit(b"merged"), commit(b"listed before"));
    graph.set_head(block, Some(merged));
    graph.listed(
        Listing::Block(block),
        vec![Node {
            head: Some(listed_before),
            ..node(block)
        }],
        3,
    );
    assert_eq!(graph.get(block).and_then(|node| node.head), Some(merged));

    // Once the block is closed, what is listed of it is its head again.
    graph.forget_reported_head(block);
    graph.listed(
        Listing::Block(block),
        vec![Node {
            head: Some(old),
            ..node(block)
        }],
        4,
    );
    assert_eq!(graph.get(block).and_then(|node| node.head), Some(old));
}
