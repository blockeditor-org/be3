use super::*;

#[tokio::test]
/// A connection must sign in with its first call, within `sign_in_timeout`; any other first call,
/// or none, closes it, and so does logging out.
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
    let mut wrong = harness.client().await;
    let first = wrong.peer.clone();
    let listed = client::organizations::list_organizations(&first).await;
    assert!(
        !matches!(listed, Ok(Ok(_))),
        "a connection was answered before it signed in: {listed:?}"
    );
    wrong.ended().await;

    let mut logging_out = harness.client().await;
    let token = harness.register("leaving@example.com", "Leaving").await;
    let response = logging_out
        .call(|peer| client::account::authenticate(peer, Some(token)))
        .await;
    assert!(response.is_ok(), "{response:?}");
    let response = logging_out.call(|peer| client::account::logout(peer, None)).await;
    assert!(matches!(response, Ok(())), "{response:?}");
    logging_out.ended().await;

    anonymous.ended().await;
    signed.round_trip().await;
    harness.stop().await;
}
