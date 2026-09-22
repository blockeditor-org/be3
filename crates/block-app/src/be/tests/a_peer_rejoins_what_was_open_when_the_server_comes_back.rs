use super::*;

#[test]
fn a_peer_rejoins_what_was_open_when_the_server_comes_back() {
    let mut harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let address = harness.address();

    open(block, CounterContent::CONTENT_TYPE);
    add(block, 4);
    wait_for_count(block, 4);
    flush();

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    assert_eq!(count_of(block), Some(4));

    harness.start_server(&address);
    wait_until("connected again", |shared| shared.connected);
    add(block, 1);

    wait_for_count(block, 5);
}
