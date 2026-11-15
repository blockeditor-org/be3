use super::*;

/// An edit, the undo of it and another edit, none of which the device heard ordered, are kept
/// in the order made: after a crash each is given again under its own number, so the last is
/// not taken for one the order holds.
#[test]
fn an_edit_an_undo_and_an_edit_kept_across_a_crash_all_land() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();

    harness.relay().hold_answers(true);
    add(block, 2);
    wait_for_count(block, 3);
    undo(block);
    wait_for_count(block, 1);
    add(block, 5);
    wait_for_count(block, 6);
    wait_until("kept the edits not heard ordered", |shared| {
        shared
            .peer
            .as_ref()
            .and_then(|peer| peer.open_edits(block))
            .is_some_and(|(_, edits)| edits.is_ok_and(|edits| edits.len() >= 3))
    });

    crash();
    platform::wait_for_stopped();
    harness.relay().hold_answers(false);

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 6);
    wait_until("had the kept edits ordered", |shared| shared.owing > 0);
    flush_and_send();
    assert_eq!(count_of(block), Some(6));
    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(6), "the edit after the undo landed too");
}
