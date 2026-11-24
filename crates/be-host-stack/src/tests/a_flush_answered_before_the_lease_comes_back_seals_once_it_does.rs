use super::*;

/// A flush answers while a peer holds the lease: this device's edit is numbered, and sealing it
/// is the holder's, so the flush owes nothing. Once this device claims the lease back (as a host
/// component does), it owes that seal, and makes it then rather than at its next quiet spell,
/// which a held clock never brings.
#[test]
fn a_flush_answered_before_the_lease_comes_back_seals_once_it_does() {
    let harness = Harness::start().on_manual_clock();
    let block = Uuid::new_v4();
    runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .ensure::<CounterContent>(block, be_block_graph::BlockParent::Root)
            .await
            .expect("the counter is created");
    });

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    let mut outside = Outside::joining(&harness, block);
    outside.claim();

    add(block, 3);
    outside.until("had this device's add", |seen| seen.count == Some(3));
    flush();
    let head = crate::node(block).and_then(|node| node.head);

    restart_host(block);
    wait_until("sealed the add once the lease came back", |shared| {
        shared
            .graph
            .get(block)
            .is_some_and(|node| node.head.is_some() && node.head != head)
            && shared.unsealed() == 0
            && shared.owing == 0
    });
    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(3));
}
