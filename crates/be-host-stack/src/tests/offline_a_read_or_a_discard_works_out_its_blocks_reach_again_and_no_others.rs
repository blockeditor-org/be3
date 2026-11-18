use super::*;

fn worked_out() -> u64 {
    with_shared(|shared| shared.reach_worked_out).unwrap_or_default()
}

/// Offline, what this device holds of a block can change without its node moving: reading a
/// version of it can record one held, and letting go of an unsent edit lets go of content. The
/// worker works out that block's reach again (`reach_dirty`), and none of the thousands of
/// others.
#[test]
fn offline_a_read_or_a_discard_works_out_its_blocks_reach_again_and_no_others() {
    let mut harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 2);
    wait_for_count(block, 2);
    flush_and_send();
    let head = node(block)
        .and_then(|node| node.head)
        .expect("the counter has a head");
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);

    let parent = Uuid::new_v4();
    let nodes: Vec<Node> = (0..2_000)
        .map(|_| Node {
            id: Uuid::new_v4(),
            content_type: CounterContent::CONTENT_TYPE,
            author: None,
            parent: Some(be_block_graph::BlockParent::Block(parent)),
            access: be_block_graph::Access::Edit,
            references: None,
            metadata: be_block_content::BlockMetadata::default(),
            head: None,
            locked: false,
            held: true,
            sets_access: false,
            size: 0,
            tree_size: 0,
            changed: 0,
            entry: None,
        })
        .collect();
    with_shared_mut(|shared| {
        shared
            .graph
            .want(Owner::Editor(u64::MAX - 3), vec![Query::Children(parent)]);
        shared.graph.listed(
            crate::graph::Listing::Children(be_block_graph::BlockParent::Block(parent)),
            nodes,
            u64::MAX,
        );
    });
    let before = worked_out();

    let (sender, receiver) = std::sync::mpsc::channel();
    read_head(block, head, move |read| {
        let _ = sender.send(read.is_ok());
    });
    assert!(
        receiver.recv().expect("the read answers"),
        "the held head reads"
    );
    let after_read = worked_out();
    assert!(
        after_read > before && after_read - before < 100,
        "the read had {} blocks' reach worked out",
        after_read - before
    );

    discard_unsent(block);
    wait_until("worked out the discarded block's reach", |shared| {
        shared.pending() == 0 && shared.reach_worked_out > after_read
    });
    assert!(
        worked_out() - after_read < 100,
        "the discard had {} blocks' reach worked out",
        worked_out() - after_read
    );
    assert!(
        !out_of_reach(block),
        "the counter is held whole, so in reach"
    );
}
