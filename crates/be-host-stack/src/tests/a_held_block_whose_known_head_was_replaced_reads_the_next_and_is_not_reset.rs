use super::*;

/// A block the host holds for itself (`hold`), opened on the head this device knows while its
/// join is on its way, may find that head's objects gone: another device published the next one
/// and the server dropped the replaced commit (retention keeps one per slot). That is no reason
/// to call the block unreadable and reset it, as content another build saved would be: the
/// replica reads again once the join tells it the head, and holds what the other device wrote.
#[test]
fn a_held_block_whose_known_head_was_replaced_reads_the_next_and_is_not_reset() {
    let harness = Harness::relayed().on_manual_clock();
    let block = Uuid::new_v4();
    // The other device's connection lives on this runtime for the whole test.
    let other = runtime();
    let outside = other.block_on(async {
        let outside = harness.outside_peer().await;
        outside
            .ensure::<CounterContent>(block, be_block_graph::BlockParent::Root)
            .await
            .expect("the block is created");
        let mut counter = CounterContent::default();
        counter.apply(&Counter::add(1));
        let first = outside
            .save(block, &counter, None)
            .await
            .expect("the first head is saved");
        let be_block_client::Saved::Published(first) = first else {
            panic!("the first head reached the server: {first:?}");
        };
        (outside, first)
    });
    let (outside, first) = outside;

    harness.connect();
    wait_until("knew the block's first head", |shared| {
        shared.graph.get(block).is_some_and(|node| node.head.is_some())
    });
    // The server answers this device nothing for now, so it hears neither the next head nor
    // its join of the block's session, while the other device replaces the head it knows: the
    // server keeps no history the organization did not ask for, so the replaced commit and its
    // objects are gone at once.
    harness.relay().hold_answers(true);
    other.block_on(async {
        let mut counter = outside
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens")
            .expect("it holds content");
        counter.apply(&Counter::add(1));
        let next = outside
            .save(block, &counter, Some(first))
            .await
            .expect("the next head is saved");
        assert!(
            matches!(next, be_block_client::Saved::Published(_)),
            "the next head replaced the first: {next:?}"
        );
    });

    // Held, it opens on the head it knows once its join is slow, which reads that head.
    let mut joins = harness.relay().count(|method, args| {
        method == be_block_protocol::__args::SERVER_READ
            && be_wasm_ops::decode::<be_block_protocol::__args::ServerReadArgs>(args)
                .is_some_and(|asked| matches!(asked.query, be_block_protocol::Query::Session { .. }))
    });
    hold(block, CounterContent::CONTENT_TYPE);
    runtime()
        .block_on(joins.wait_for(|joins| *joins > 0))
        .expect("holding the block asked to join its session");
    harness.clock.advance(be_block_type_host::EARLY_OPEN);
    harness.relay().hold_answers(false);

    wait_for_count(block, 2);
    assert!(
        !be_host_base::notices::shown()
            .iter()
            .any(|notice| notice.message.contains("were reset")),
        "a block whose known head was replaced was reset"
    );
}
