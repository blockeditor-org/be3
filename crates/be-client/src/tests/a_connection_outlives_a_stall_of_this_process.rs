use super::*;
use crate::connection::{PING_INTERVAL, PONG_WAIT};
use be_protocol::ClientMessage;

#[tokio::test]
async fn a_connection_outlives_a_stall_of_this_process() {
    let harness = Harness::start().await;
    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = fault_relay::Relay::start(server);
    let clock = Clock::manual();
    let connection = Connection::connect(&relay.url(), &clock).await.unwrap();
    let mut pings = relay.count(|message| matches!(message, ClientMessage::Ping { .. }));

    clock.advance(PING_INTERVAL + PONG_WAIT + PONG_WAIT);
    let answered = connection
        .request(|request| ClientMessage::ListOrganizations { request })
        .await;
    assert!(
        !answered.is_err_and(|error| error.is_unreachable()),
        "the request was answered after the stall"
    );
    pings.wait_for(|pings| *pings > 0).await.unwrap();

    clock.advance(PONG_WAIT);
    let answered = connection
        .request(|request| ClientMessage::ListOrganizations { request })
        .await;
    assert!(
        !answered.is_err_and(|error| error.is_unreachable()),
        "the connection kept going after the server answered its ping"
    );
    assert!(!connection.is_closed());
}
