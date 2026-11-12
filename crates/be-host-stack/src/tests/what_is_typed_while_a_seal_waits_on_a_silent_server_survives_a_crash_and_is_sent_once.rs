use super::*;

#[test]
fn what_is_typed_while_a_seal_waits_on_a_silent_server_survives_a_crash_and_is_sent_once() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();

    let mut sending = harness.relay().count(|method| {
        [
            be_block_protocol::__args::OBJECTS_MISSING_OBJECTS,
            be_block_protocol::__args::OBJECTS_UPLOAD_URLS,
            be_block_protocol::__args::BLOCKS_PUBLISH,
        ]
        .contains(&method)
    });
    harness.relay().stall();
    add(block, 2);
    wait_for_count(block, 3);
    harness
        .clock
        .advance(Duration::from_millis(be_sdk_type_guest::QUIET_INTERVAL));
    runtime()
        .block_on(sending.wait_for(|sent| *sent > 0))
        .expect("the counter's owner sent its seal");
    add(block, 4);
    wait_for_count(block, 7);
    // Alone in its session, the owner appends what it sequenced once `APPEND_INTERVAL` passes.
    wait_until("counted the edit as not yet journaled", |shared| {
        shared.undurable > 0
    });
    harness
        .clock
        .advance(Duration::from_millis(be_sdk_type_guest::APPEND_INTERVAL));
    wait_until("journaled the edit made while the seal was out", |shared| {
        shared.undurable == 0
    });

    // Nothing is kept on the way out: the seal waits in the outbox and the later edit only in
    // the journal, which notes the seal it follows.
    crash();
    platform::wait_for_stopped();

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 7);
    wait_until("sent what this device kept", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    flush_and_send();
    assert_eq!(count_of(block), Some(7));
    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(7));
}
