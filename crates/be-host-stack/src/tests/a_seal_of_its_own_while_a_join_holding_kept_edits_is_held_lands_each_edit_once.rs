use super::*;

/// How many local edits the host keeps of `block` on the device, to give its replica again.
fn kept_edits(shared: &Shared, block: Uuid) -> usize {
    shared
        .peer
        .as_ref()
        .and_then(|peer| peer.open_edits(block))
        .map_or(0, |(_, edits)| edits.map_or(0, |(edits, _)| edits.len()))
}

/// A block reopened after a crash with edits kept on the device joins its session holding
/// them. While the join's answer is held (`be_block_type_host::JOIN_PAUSE`), its made-up session
/// takes another edit and seals it as it closes: that commit waits in the outbox as the replica takes
/// the server's journal, and once a drain sends it the replica merges it, each edit landing once.
#[test]
fn a_seal_of_its_own_while_a_join_holding_kept_edits_is_held_lands_each_edit_once() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    let key = block.as_u128();
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

    be_lib_test_pause::hold(be_block_type_host::JOIN_PAUSE, key);
    harness.relay().offline(false);
    retry_now();
    runtime().block_on(be_lib_test_pause::reached(
        be_block_type_host::JOIN_PAUSE,
        key,
        1,
    ));
    // The made-up session takes an edit and seals it as the block closes, the join still held.
    let joined_at = crate::with_shared(|shared| {
        shared.peer.as_ref().and_then(|peer| peer.local_head(block))
    })
    .flatten();
    add(block, 8);
    wait_for_count(block, 15);
    close(block);
    wait_until("sealed in the made-up session while the join was held", |shared| {
        shared.owing == 0
            && shared
                .peer
                .as_ref()
                .is_some_and(|peer| peer.local_head(block) != joined_at)
    });
    be_lib_test_pause::release(be_block_type_host::JOIN_PAUSE, key);
    wait_until("closed the counter", |shared| !shared.blocks.contains_key(&block));
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 15);

    wait_until("rejoined and sent what this device kept", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    add(block, 16);
    wait_for_count(block, 31);
    flush_and_send();
    assert_eq!(count_of(block), Some(31));
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
    assert_eq!(saved, Some(31), "each edit landed once");
}
