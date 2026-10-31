use super::*;
use crate::connection::{PING_INTERVAL, PONG_WAIT};
use be_protocol::client;

#[tokio::test]
async fn a_request_stuck_in_a_full_socket_fails_once_the_server_goes_silent() {
    let harness = Harness::start().await;
    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = fault_relay::Relay::start(server);
    let clock = Clock::manual();
    let connection = Connection::connect(&relay.url(), &clock).await.unwrap();

    relay.jam_at_next_request();
    let heard = relay.heard();
    let answer = crate::ask(client::objects::put_objects(
        &connection,
        vec![vec![7; be_protocol::MAX_FRAME_BYTES - 1024]],
    ));
    relay.heard_beyond(heard).await;
    clock.advance(PING_INTERVAL + PONG_WAIT);

    assert!(
        matches!(answer.await, Err(ClientError::Disconnected(_))),
        "the request failed once the server stopped taking what was sent"
    );
    assert!(connection.is_closed());
}
