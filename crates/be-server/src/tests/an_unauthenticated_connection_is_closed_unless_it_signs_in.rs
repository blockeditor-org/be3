use super::*;

#[tokio::test]
async fn an_unauthenticated_connection_is_closed_unless_it_signs_in() {
    let harness = Harness::configured(ServerConfig {
        sign_in_timeout: Duration::from_secs(3),
        ..ServerConfig::OPEN
    })
    .await;
    let token = harness.register("signed@example.com", "Signed").await;
    let mut anonymous = harness.client().await;
    let mut signed = harness.client().await;
    let response = signed
        .call(|peer| client::account::authenticate(peer, Some(token)))
        .await;
    assert!(response.is_ok(), "{response:?}");
    let mut signing = harness.client().await;
    begin_session(&mut signing).await;

    anonymous.ended().await;
    signed.round_trip().await;
    signing.round_trip().await;
    harness.stop().await;
}
