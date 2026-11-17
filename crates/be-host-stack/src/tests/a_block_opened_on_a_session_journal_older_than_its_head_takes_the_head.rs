use super::*;

/// This device seals an edit offline onto the head the server's session journal is based on,
/// and the drain publishes it while no replica is in the session to move that journal on.
/// Opened again, the replica starts on the journal's base and must hear of the newer head, or it
/// shows the base (a count of 4) for good.
#[test]
fn a_block_opened_on_a_session_journal_older_than_its_head_takes_the_head() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    add(block, 4);
    wait_for_count(block, 4);
    flush_and_send();
    close(block);
    wait_until("closed the counter and left its session", |shared| {
        !shared.blocks.contains_key(&block) && !shared.leaving
    });

    harness.relay().offline(true);
    wait_until("noticed the server was gone", |shared| !shared.connected);
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 4);
    add(block, 2);
    wait_for_count(block, 6);
    flush();
    close(block);
    wait_until("closed the counter offline", |shared| {
        !shared.blocks.contains_key(&block) && shared.waiting > 0
    });

    harness.relay().offline(false);
    retry_now();
    wait_until("reconnected and published the edit", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });

    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 6);
}
