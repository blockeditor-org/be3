use super::*;
use crate::connection::{PING_INTERVAL, PONG_WAIT};
use be_protocol::ClientMessage;

#[tokio::test]
async fn a_request_fails_once_a_ping_goes_unanswered() {
    let harness = Harness::start().await;
    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = fault_relay::Relay::start(server);
    let clock = Clock::manual();
    let connection = Connection::connect(&relay.url(), &clock).await.unwrap();

    relay.stall();
    let answer = connection
        .send(|request| ClientMessage::ListOrganizations { request })
        .unwrap();
    let mut pings = relay.count(|message| matches!(message, ClientMessage::Ping { .. }));
    clock.advance(PING_INTERVAL);
    pings.wait_for(|pings| *pings > 0).await.unwrap();
    clock.advance(PONG_WAIT);

    assert!(
        matches!(answer.await, Err(ClientError::Disconnected(_))),
        "the request failed once the server missed a ping"
    );
    assert!(connection.is_closed());
}
