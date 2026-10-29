use super::*;

#[test]
fn an_edit_made_offline_survives_a_restart_and_reaches_the_server_on_reconnect() {
    let mut harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let address = harness.address();

    open(block, CounterContent::CONTENT_TYPE);
    add(block, 2);
    wait_for_count(block, 2);
    flush();

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    add(block, 3);
    wait_for_count(block, 5);
    stop();

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 5);
    assert!(
        status().waiting > 0,
        "the edits sealed offline were not waiting to sync"
    );
    assert!(!status().connected);
    add(block, 1);
    wait_for_count(block, 6);
    flush();

    harness.start_server(&address);
    retry_now();
    wait_until("reconnected and sent what waited", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    flush();

    let saved = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(6));
}
