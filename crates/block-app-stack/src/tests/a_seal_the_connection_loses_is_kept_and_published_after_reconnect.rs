use super::*;

#[test]
fn a_seal_the_connection_loses_is_kept_and_published_after_reconnect() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    add(block, 4);
    wait_for_count(block, 4);
    flush();

    harness.relay().stall();
    add(block, 3);
    wait_until("owes a seal for the edit", |shared| shared.owing > 0);
    harness
        .clock
        .advance(Duration::from_millis(block_type_guest::SEAL_INTERVAL));
    wait_until("sealed the edit while the connection hung", |shared| {
        shared.waiting > 0
    });
    harness.miss_a_ping();
    wait_until("noticed the connection went quiet", |shared| {
        !shared.connected
    });
    retry_now();
    wait_until("reconnected and sent the kept seal", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    assert_eq!(count_of(block), Some(7));
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
