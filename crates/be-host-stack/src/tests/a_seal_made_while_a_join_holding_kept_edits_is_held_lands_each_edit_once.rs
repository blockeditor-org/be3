use super::*;

/// How many local edits the host keeps of `block` on the device, to give its replica again.
fn kept_edits(shared: &Shared, block: Uuid) -> usize {
    shared
        .peer
        .as_ref()
        .and_then(|peer| peer.open_edits(block))
        .map_or(0, |(_, edits)| edits.map_or(0, |(edits, _)| edits.len()))
}

/// A block reopened offline after a crash holds an edit its journal says the server ordered,
/// which no seal holds, and one the device kept that the server never answered. Reconnecting,
/// its made-up session seals what it holds while its join is on its way (held here once the
/// server answered), and an edit is made meanwhile: once the join goes on, each edit lands
/// once, none lost and none applied twice.
#[test]
fn a_seal_made_while_a_join_holding_kept_edits_is_held_lands_each_edit_once() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();

    // Ordered and in this device's journal, with no seal holding it.
    add(block, 2);
    wait_for_count(block, 3);
    wait_until("journaled the ordered edit", |shared| {
        shared.owing > 0 && shared.undurable == 0
    });

    harness.relay().stall();
    add(block, 4);
    wait_for_count(block, 7);
    wait_until("kept the edit the server has not answered", |shared| {
        kept_edits(shared, block) >= 1
    });

    crash();
    platform::wait_for_stopped();

    harness.relay().offline(true);
    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 7);

    let paused = std::sync::Arc::new(tokio::sync::Notify::new());
    let resume = std::sync::Arc::new(tokio::sync::Notify::new());
    be_block_type_host::replicas::pause_rejoin(block, paused.clone(), resume.clone());
    harness.relay().offline(false);
    retry_now();
    runtime().block_on(paused.notified());

    // The join is held with the server's answer: the made-up session seals the ordered edit
    // into the outbox, and an edit is made meanwhile.
    wait_until("sealed what the made-up session held", |shared| {
        shared
            .peer
            .as_ref()
            .is_some_and(|peer| !peer.pending_commits(block).is_empty())
    });
    add(block, 8);
    wait_for_count(block, 15);

    resume.notify_one();
    wait_until("rejoined and sent what this device kept", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    flush_and_send();
    assert_eq!(count_of(block), Some(15));
    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(15), "each edit landed once");
}
