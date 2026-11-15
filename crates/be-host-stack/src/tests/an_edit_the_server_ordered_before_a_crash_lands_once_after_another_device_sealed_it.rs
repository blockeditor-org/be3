use super::*;

/// The server numbers an edit but its answers never arrive; another device, which stays in the
/// session, seals it, so the journal no longer holds its entry; then this device crashes. Opened again, it finds in the
/// journal's base how far the order holds its edits: the edit lands once, and the one made
/// after it, which never reached the server, is sent.
#[test]
fn an_edit_the_server_ordered_before_a_crash_lands_once_after_another_device_sealed_it() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();
    let mut outside = Outside::joining(&harness, block);
    outside.until("read the counter", |seen| seen.count == Some(1));

    harness.relay().hold_answers(true);
    add(block, 2);
    add(block, 4);
    wait_for_count(block, 7);
    outside.until("heard the edit the server ordered", |seen| {
        seen.count == Some(3)
    });
    wait_until("kept the edits not heard ordered", |shared| {
        shared
            .peer
            .as_ref()
            .and_then(|peer| peer.open_edits(block))
            .is_some_and(|(_, edits)| edits.is_ok_and(|edits| edits.len() >= 2))
    });
    outside.claim();
    outside.seal();

    crash();
    platform::wait_for_stopped();
    harness.relay().hold_answers(false);

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 7);
    wait_until("had the edit the server lacked ordered", |shared| {
        shared.undurable == 0
    });
    assert_eq!(count_of(block), Some(7));
    // The other device holds the lease, and seals what the order holds.
    outside.until("heard the edit sent again", |seen| seen.count == Some(7));
    outside.seal();
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
    outside.leave();
}
