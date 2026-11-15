use super::*;

/// What the page reads survives being handed over as bytes, as it will be to a page whose
/// memory the worker does not share: the graph, an open block's content, its history and the
/// counts a status is made of read the same from the decoded `Seen`.
#[test]
fn a_seen_reads_the_same_from_its_bytes() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 3);
    wait_for_count(block, 3);
    flush_and_send();

    let seen = with_shared(|shared| shared.seen.clone()).expect("the stack runs");
    let read = crate::Seen::decode(&seen.encode()).expect("it decodes");

    let nodes = |seen: &crate::Seen| {
        let mut nodes: Vec<Node> = seen.graph.nodes().map(|node| Node::clone(node)).collect();
        nodes.sort_by_key(|node| node.id);
        nodes
    };
    assert_eq!(nodes(&read), nodes(&seen));
    assert_eq!(read.graph.query(Query::Roots), seen.graph.query(Query::Roots));
    assert_eq!(read.graph.ready(Query::Roots), seen.graph.ready(Query::Roots));
    assert_eq!(
        read.blocks.get(&block).map(|content| content.bytes().to_vec()),
        seen.blocks.get(&block).map(|content| content.bytes().to_vec())
    );
    assert_eq!(read.histories.get(&block), seen.histories.get(&block));
    assert_eq!(
        (read.connected, read.saves, read.done, read.waiting, read.edits_applied),
        (seen.connected, seen.saves, seen.done, seen.waiting, seen.edits_applied)
    );
    assert_eq!(read.unsynced(), seen.unsynced());
}
