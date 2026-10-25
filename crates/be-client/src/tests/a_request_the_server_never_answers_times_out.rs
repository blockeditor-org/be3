use super::*;
use crate::connection::REQUEST_TIMEOUT;
use be_protocol::ClientMessage;

#[tokio::test]
async fn a_request_the_server_never_answers_times_out() {
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
    let heard = relay.heard();
    let answer = connection
        .send(|request| ClientMessage::ListOrganizations { request })
        .unwrap();
    relay.heard_beyond(heard).await;
    clock.advance(REQUEST_TIMEOUT);

    assert!(
        matches!(answer.await, Err(ClientError::TimedOut(_))),
        "the request gave up once its time ran out"
    );
}
