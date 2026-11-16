use super::*;

/// The host submits an edit and crashes while the submission is still on its way. Opened again,
/// it gives the edit again, and the server orders it; only then does the first submission
/// reach the server. The edit went again as exactly that submission, under its id, so the
/// server holds it already and the edit lands once.
#[test]
fn an_edit_submitted_before_a_crash_that_reaches_the_server_after_it_is_given_again_lands_once() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();
    let mut outside = Outside::joining(&harness, block);
    outside.until("read the counter", |seen| seen.count == Some(1));

    harness.relay().hold_requests(true);
    add(block, 10);
    wait_for_count(block, 11);
    wait_until("kept the edit and its submission on the device", |shared| {
        shared.undurable == 0
    });
    crash();
    platform::wait_for_stopped();

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    outside.until("heard the edit given again", |seen| seen.count == Some(11));
    // The first submission reaches the server now, after the one given again.
    harness.relay().hold_requests(false);
    add(block, 100);
    outside.until("heard the edit typed after", |seen| {
        seen.count.is_some_and(|count| count >= 111)
    });
    wait_for_count(block, 111);
    flush_and_send();
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
    assert_eq!(saved, Some(111), "each edit landed once");
    outside.leave();
}
