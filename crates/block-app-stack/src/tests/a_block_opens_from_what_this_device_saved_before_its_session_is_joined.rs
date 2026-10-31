use super::*;

#[test]
fn a_block_opens_from_what_this_device_saved_before_its_session_is_joined() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    add(block, 4);
    wait_for_count(block, 4);
    flush();
    close(block);
    wait_until("closed the counter", |shared| {
        !shared.blocks.contains_key(&block)
    });

    harness.relay().stall();
    harness.open_until_it_joins(block, CounterContent::CONTENT_TYPE);
    harness.clock.advance(be_types_host::EARLY_OPEN);
    wait_for_count(block, 4);
    add(block, 2);
    wait_for_count(block, 6);
    assert!(
        status().connected,
        "the block opened and took an edit before the connection was given up"
    );
    harness.miss_a_ping();
    wait_until("noticed the connection went quiet", |shared| {
        !shared.connected
    });
    wait_for_count(block, 6);
    wait_until("owes a seal for the edit", |shared| shared.owing > 0);
    harness
        .clock
        .advance(Duration::from_millis(block_type_guest::SEAL_INTERVAL));
    wait_until("sealed the edit made before joining", |shared| {
        shared.waiting > 0
    });
    retry_now();
    wait_until("reconnected and sent the edit", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });

    close(block);
    wait_until("closed the counter again", |shared| {
        !shared.blocks.contains_key(&block)
    });
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 6);
    add(block, 1);
    wait_for_count(block, 7);
    flush();

    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(7));
}
