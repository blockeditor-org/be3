use super::*;

#[test]
fn typing_shows_while_a_silent_server_holds_a_seal_and_stopping_keeps_it_here() {
    let harness = Harness::relayed();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush();

    let mut sending = harness.relay().count(|message| {
        matches!(
            message,
            be_protocol::ClientMessage::MissingObjects { .. }
                | be_protocol::ClientMessage::PutObjects { .. }
                | be_protocol::ClientMessage::Publish { .. }
        )
    });
    harness.relay().stall();
    add(block, 2);
    wait_for_count(block, 3);
    harness
        .clock
        .advance(Duration::from_millis(block_type_guest::SEAL_INTERVAL));
    let sent = runtime().block_on(async {
        tokio::time::timeout(PATIENCE, sending.wait_for(|sent| *sent > 0))
            .await
            .is_ok_and(|seen| seen.is_ok())
    });
    assert!(sent, "the counter's owner never sent its seal");
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
    assert!(
        stopping.recv_timeout(PATIENCE).is_ok(),
        "stopping waited on the silent server"
    );

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
