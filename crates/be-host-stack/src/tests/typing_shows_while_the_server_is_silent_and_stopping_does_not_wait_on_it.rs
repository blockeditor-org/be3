use super::*;

/// Edits show at once while the server says nothing, and stopping does not wait on it longer
/// than a flush does. The device keeps the edits, and sends them once it is back.
#[test]
fn typing_shows_while_the_server_is_silent_and_stopping_does_not_wait_on_it() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    crate::flush_and_send();

    harness.relay().stall();
    add(block, 2);
    add(block, 4);
    wait_for_count(block, 7);
    assert!(
        status().connected,
        "the edits showed only once the connection was given up"
    );
    assert!(status().unsaved > 0, "what the server has not ordered is unsaved");

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
    // The kept edits are submitted again and owed a seal once the server ordered them.
    wait_until("had the kept edits ordered", |shared| shared.owing > 0);
    flush_and_send();
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
