use super::*;

/// How many local edits the host keeps of `block` on the device, to give its replica again.
fn kept_edits(shared: &Shared, block: Uuid) -> usize {
    shared
        .peer
        .as_ref()
        .and_then(|peer| peer.open_edits(block))
        .map_or(0, |(_, edits)| edits.map_or(0, |(edits, _)| edits.len()))
}

/// Edits made while the server takes submissions and says nothing are kept on the device as
/// they are made. After a crash the device opens offline and shows them; once it reconnects,
/// what the server's order held already is found there and the rest is sent, so each lands once.
#[test]
fn what_is_typed_while_a_seal_waits_on_a_silent_server_survives_a_crash_and_is_sent_once() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();

    harness.relay().stall();
    add(block, 2);
    add(block, 4);
    wait_for_count(block, 7);
    wait_until("kept the edits the server has not ordered", |shared| {
        kept_edits(shared, block) >= 2
    });

    crash();
    platform::wait_for_stopped();

    harness.relay().offline(true);
    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 7);
    assert!(
        !status().connected,
        "the edits show while the device is still offline"
    );

    harness.relay().offline(false);
    retry_now();
    wait_until("rejoined and sent what this device kept", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    // Rejoined, the replica submits what the server's order lacks, and owes a seal of it.
    wait_until("had the kept edits ordered", |shared| shared.owing > 0);
    flush_and_send();
    assert_eq!(count_of(block), Some(7));
    assert_eq!(
        crate::with_shared(|shared| kept_edits(shared, block)),
        Some(0),
        "nothing is left to give again"
    );
    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(7), "each edit landed once");
}
