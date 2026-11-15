use super::*;

/// Edits show at once while the server says nothing, and stopping does not wait on it. What it
/// never ordered is lost (decisions/live_edits_are_ordered_by_the_server.md, Accepted limits):
/// the device keeps what the server held before it went silent.
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
    wait_for_count(block, 1);
    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(1));
}
