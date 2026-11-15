use super::*;

/// An edit and the undo of it, made while the server takes submissions and says nothing, are
/// both kept on the device: after a crash the device opens offline showing neither, and once it
/// reconnects the server holds the edit undone, not the edit alone.
#[test]
fn an_undo_made_while_the_server_is_silent_survives_a_crash() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();

    harness.relay().stall();
    add(block, 2);
    wait_for_count(block, 3);
    undo(block);
    wait_for_count(block, 1);
    wait_until("kept the edit and its undo", |shared| {
        shared
            .peer
            .as_ref()
            .and_then(|peer| peer.open_edits(block))
            .is_some_and(|(_, edits)| edits.is_ok_and(|edits| edits.len() >= 2))
    });

    crash();
    platform::wait_for_stopped();

    harness.relay().offline(true);
    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 1);

    harness.relay().offline(false);
    retry_now();
    wait_until("rejoined", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    wait_until("had the kept edits ordered", |shared| shared.owing > 0);
    flush_and_send();
    assert_eq!(count_of(block), Some(1));
    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(1), "the undo landed with the edit it undoes");
}
