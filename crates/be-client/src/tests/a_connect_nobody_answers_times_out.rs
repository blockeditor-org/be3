use super::*;
use crate::connection::CONNECT_TIMEOUT;

#[tokio::test]
async fn a_connect_nobody_answers_times_out() {
    let relay = fault_relay::Relay::start("127.0.0.1:9".parse().unwrap());
    relay.hold_new(true);
    let clock = Clock::manual();
    let connecting = tokio::spawn({
        let url = relay.url();
        let clock = clock.clone();
        async move { Connection::connect(&url, &clock).await.map(drop) }
    });

    relay.heard_beyond(0).await;
    clock.advance(CONNECT_TIMEOUT);

    assert!(matches!(
        connecting.await.unwrap(),
        Err(ClientError::TimedOut(_))
    ));
}
