use super::*;

#[test]
fn stopping_returns_while_a_connect_goes_unanswered() {
    let harness = Harness::relayed();
    harness.connect();
    wait_until("connected", |shared| shared.connected);

    harness.relay().hold_new(true);
    harness.relay().cut();
    wait_until("noticed the connection was cut", |shared| !shared.connected);
    let heard = harness.relay().heard();
    retry_now();
    runtime()
        .block_on(async {
            tokio::time::timeout(PATIENCE, harness.relay().heard_beyond(heard)).await
        })
        .expect("the peer tried to connect again");

    stop();
    assert!(!status().running);
}
