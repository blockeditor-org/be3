use super::*;

#[test]
fn typing_shows_while_a_silent_server_holds_a_seal_and_stopping_keeps_it_here() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    // The server has the first seal before the relay goes silent: one still being sent then
    // would hold the outbox, and the seal this counts would never be sent at all.
    crate::flush_and_send();

    let mut sending = harness.relay().count(|method, _| {
        [
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
    let sent = runtime().block_on(async {
        tokio::time::timeout(crate::WAIT_LIMIT, sending.wait_for(|sent| *sent > 0))
            .await
            .map(|sent| sent.is_ok())
    });
    let now = status();
    assert!(
        sent == Ok(true),
        "the counter's owner never sent its seal (connected: {}, unsaved: {}, kept: {}, wakes: {})",
        now.connected,
        now.unsaved,
        now.kept,
        now.wakes
    );
    add(block, 4);
    wait_for_count(block, 7);
    assert!(
        status().connected,
        "the edit showed only once the connection was given up"
    );

    let (stopped, stopping) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        stop_and_wait();
        let _ = stopped.send(());
    });
    stopping
        .recv_timeout(crate::WAIT_LIMIT)
        .expect("stopping returned without waiting on the silent server");

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 7);
    wait_until("sent what this device kept", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
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
