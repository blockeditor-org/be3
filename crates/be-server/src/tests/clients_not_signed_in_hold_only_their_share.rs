use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use super::*;

/// Connects as if from `address`, through the trusted proxy every test connects from.
async fn from(harness: &Harness, address: &str) -> Result<TestClient, tokio_tungstenite::tungstenite::Error> {
    let mut request = harness.socket().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("x-forwarded-for", address.parse().unwrap());
    let (mut socket, _) = connect_async(request).await?;
    socket.send(WsMessage::Binary(hello(BUILD))).await.unwrap();
    assert_eq!(
        next_binary(&mut socket).await.as_deref().and_then(read_hello).as_deref(),
        Some(BUILD)
    );
    Ok(TestClient::over(socket))
}

/// One address holds only so many connections that are not signed in; a browser sign-in keeps a
/// connection open only while the sign-in is still its own.
#[tokio::test]
async fn clients_not_signed_in_hold_only_their_share() {
    let harness = Harness::configured(ServerConfig {
        sign_in_timeout: Duration::from_secs(3),
        ..ServerConfig::OPEN
    })
    .await;
    let token = harness.register("signed@example.com", "Signed").await;

    let mut crowd = Vec::new();
    for _ in 0..crate::MAX_UNSIGNED_CONNECTIONS_PER_ADDRESS {
        crowd.push(from(&harness, "203.0.113.7").await.expect("a connection was refused"));
    }
    assert!(
        from(&harness, "203.0.113.7").await.is_err(),
        "an address held more connections that are not signed in than it may"
    );
    let mut signed = from(&harness, "198.51.100.1").await.expect("another address was refused");
    let response = signed
        .call(|peer| client::account::authenticate(peer, Some(token)))
        .await;
    assert!(response.is_ok(), "{response:?}");
    let mut began = crowd.pop().unwrap();

    let (code, secret) = begin_session(&mut began).await;
    let mut taker = from(&harness, "198.51.100.2").await.unwrap();
    let waiting = client::account::finish_browser_login(&taker.peer, code, secret);
    taker.round_trip().await;
    began.ended().await;
    taker.round_trip().await;
    drop(waiting);
    signed.round_trip().await;
    harness.stop().await;
}
