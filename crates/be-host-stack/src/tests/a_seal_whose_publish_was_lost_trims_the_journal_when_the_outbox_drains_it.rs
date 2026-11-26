use super::*;

/// A seal whose publish the server never took stays in the outbox, and seals made offline
/// after it build on it. Drained on reconnecting, it carries its journal seal and trims the
/// server's journal as it wins the head, so a replica opened again starts after the edit it
/// holds and does not apply it a second time.
#[test]
fn a_seal_whose_publish_was_lost_trims_the_journal_when_the_outbox_drains_it() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    add(block, 2);
    wait_for_count(block, 2);
    wait_until("the server ordered the edit", |shared| shared.owing > 0);

    // The server goes silent before the seal's publish reaches it.
    harness.relay().stall();
    flush();

    // Its publish is lost with the connection, and it waits in the outbox.
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
    wait_until("reconnected and drained the outbox", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });

    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 5);
    flush_and_send();
    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(
        saved,
        Some(5),
        "the edit the lost seal held was applied twice"
    );
}
