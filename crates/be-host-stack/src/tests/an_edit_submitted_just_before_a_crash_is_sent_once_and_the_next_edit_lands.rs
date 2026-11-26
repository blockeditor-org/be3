use super::*;

/// The server numbers an edit but its answer never arrives, and the host crashes right after it
/// submitted, before an idle pass could keep anything: that it submitted the edit was written
/// before the submission went out, so opened again the host neither makes the edit twice nor
/// takes the next one typed for one the order holds already.
#[test]
fn an_edit_submitted_just_before_a_crash_is_sent_once_and_the_next_edit_lands() {
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
    outside.until("heard the edit the server ordered", |seen| {
        seen.count == Some(3)
    });
    crash();
    platform::wait_for_stopped();
    harness.relay().hold_answers(false);

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 3);
    add(block, 4);
    wait_for_count(block, 7);
    wait_until("the server ordered the edit typed after", |shared| {
        shared.undurable == 0
    });
    outside.until("heard the edit typed after", |seen| seen.count == Some(7));
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
