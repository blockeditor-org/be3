use super::*;

/// The server orders an edit and loses the seal that holds it, so its session journal stays on
/// the base the seal was made on, entry and all. Offline, this device seals another edit onto
/// that seal, and the outbox publishes both. Opened again, the replica must open on the seal the
/// head descends from, not on the journal's base, or it merges the head in on top of the entry
/// the head already holds and counts the first edit twice (a count of 7).
#[test]
fn an_edit_sealed_offline_onto_a_seal_the_server_lost_lands_once() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    add(block, 2);
    // Owed once the server ordered it; the connection hangs only then, as the seal goes out.
    wait_until("owes a seal for the edit", |shared| shared.owing > 0);
    // What the seal sends the server is lost: the journal's move onto it and its publish.
    harness.relay().stall();
    flush();

    harness.relay().offline(true);
    wait_until("noticed the server was gone", |shared| !shared.connected);
    add(block, 3);
    wait_for_count(block, 5);
    flush();
    close(block);
    wait_until("closed the counter offline", |shared| {
        !shared.blocks.contains_key(&block) && shared.waiting > 0
    });

    harness.relay().offline(false);
    retry_now();
    wait_until("reconnected and published both seals", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    open(block, CounterContent::CONTENT_TYPE);
    wait_until("opened the counter in its session", |shared| {
        shared.blocks.contains_key(&block) && !shared.previewed.contains(&block)
    });
    // It opens on the seal the head descends from (a count of 2) and is told of the head after,
    // so it counts 5 only once it took the head.
    wait_for_count(block, 5);
    // Settled: a merge that counted the first edit again would move on to 7.
    flush_and_send();
    assert_eq!(count_of(block), Some(5));

    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(5));
}
