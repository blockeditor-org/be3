use super::*;
use crate::connection::{ANSWER_WAIT, PING_INTERVAL, QUICK_PONG_WAIT};
use be_block_protocol::client;

/// A request that hears nothing back pings at once instead of at the idle ping, and the
/// connection closes once that ping goes unanswered too: well before an idle connection would.
#[tokio::test]
async fn a_request_on_a_link_gone_silent_fails_without_waiting_for_the_idle_ping() {
    let harness = Harness::start().await;
    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = be_block_fault_relay::Relay::start(server);
    let clock = Clock::manual();
    let connection = Connection::connect(&relay.url(), &clock).await.unwrap();

    relay.stall();
    let answer = crate::ask(client::organizations::list_organizations(&connection));
    let mut pings = relay.count(|method| method == be_block_protocol::__args::CONNECTION_PING);
    let mut due = connection.due();
    due.wait_for(|due| *due < clock.now() + PING_INTERVAL)
        .await
        .unwrap();
    clock.advance(ANSWER_WAIT);
    pings.wait_for(|pings| *pings > 0).await.unwrap();
    clock.advance(QUICK_PONG_WAIT);

    assert!(
        matches!(answer.await, Err(ClientError::Disconnected(_))),
        "the request failed once the server missed the early ping"
    );
    assert!(connection.is_closed());
}
