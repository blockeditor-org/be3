use super::*;

/// A listing the server answered before a block was made, and that lands after, does not take
/// the block out while its creation still waits to be sent: the answer is laid over with the
/// outbox as it stands when it lands.
#[test]
fn a_block_made_while_a_listing_is_answered_stays_listed() {
    let harness = Harness::relayed();
    harness.connect();
    ask_for(vec![Query::Roots]);
    wait_until("listed the roots", |shared| {
        shared.graph.ready(Query::Roots)
    });

    let mut listed = harness
        .relay()
        .count(|method, _| method == be_block_protocol::__args::BLOCKS_LIST_BLOCKS);
    let before = *listed.borrow_and_update();
    harness.relay().hold_answers(true);
    refetch(Query::Roots);
    runtime()
        .block_on(listed.wait_for(|asked| *asked > before))
        .expect("the roots were asked for again");
    // The server answers from before the block; its creation never reaches it.
    harness.relay().hold_requests(true);

    let block = Uuid::new_v4();
    create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Made meanwhile"),
        None,
        false,
    );
    wait_until("listed the new block", |shared| {
        shared.graph.get(block).is_some()
    });
    flush();
    harness.relay().hold_answers(false);
    wait_until("took the answer from before", |shared| {
        shared.graph.ready(Query::Roots)
    });
    assert!(
        query(Query::Roots).iter().any(|node| node.id == block),
        "an answer from before the block took it out while its creation waits"
    );
}
