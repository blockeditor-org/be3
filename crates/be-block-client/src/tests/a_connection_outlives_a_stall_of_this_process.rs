use super::*;
use crate::connection::{PING_INTERVAL, PONG_WAIT};

#[tokio::test]
async fn a_connection_outlives_a_stall_of_this_process() {
    let harness = Harness::start().await;
    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = be_block_fault_relay::Relay::start(server);
    let clock = Clock::manual();
    let token = harness.server.register("stall@example.com", "Stall").await;
    let (connection, _) = Connection::connect(&relay.url(), &clock, &Credentials::Token(token))
        .await
        .unwrap();
    let mut pings = relay.count(|method, _| method == be_block_protocol::__args::SERVER_PING);

    clock.advance(PING_INTERVAL + PONG_WAIT + PONG_WAIT);
    let answered = crate::ask(be_block_protocol::ask::<be_block_protocol::ServerInfo, _>(
        &connection,
        None,
        be_block_protocol::Query::ServerInfo,
        false,
    ))
    .await;
    assert!(
        !answered.is_err_and(|error| error.is_unreachable()),
        "the request was answered after the stall"
    );
    pings.wait_for(|pings| *pings > 0).await.unwrap();

    clock.advance(PONG_WAIT);
    let answered = crate::ask(be_block_protocol::ask::<be_block_protocol::ServerInfo, _>(
        &connection,
        None,
        be_block_protocol::Query::ServerInfo,
        false,
    ))
    .await;
    assert!(
        !answered.is_err_and(|error| error.is_unreachable()),
        "the connection kept going after the server answered its ping"
    );
    assert!(!connection.is_closed());
}
