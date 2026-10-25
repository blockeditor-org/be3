use be_client::connection::{PING_INTERVAL, PONG_WAIT};

use super::*;

#[test]
fn a_half_open_connection_is_noticed_and_the_peer_reconnects() {
    let harness = Harness::relayed();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    add(block, 4);
    wait_for_count(block, 4);
    flush();

    harness.relay().stall();
    harness.clock.advance(PING_INTERVAL + PONG_WAIT);
    wait_until("noticed the connection went quiet", |shared| {
        !shared.connected
    });
    assert_eq!(count_of(block), Some(4));

    retry_now();
    wait_until("connected again", |shared| shared.connected);
    add(block, 1);

    wait_for_count(block, 5);
}
