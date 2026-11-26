use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use super::*;

/// A socket as if from `address`, through the trusted proxy every test connects from, that has
/// not greeted the server yet.
// tungstenite's own error, as connecting gives it.
#[allow(clippy::result_large_err)]
async fn from(
    harness: &Harness,
    address: &str,
) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
    let mut request = harness.socket().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("x-forwarded-for", address.parse().unwrap());
    Ok(connect_async(request).await?.0)
}

/// Signs `socket` in with `token`, in its greeting.
async fn signed_in(mut socket: Socket, token: &str) -> TestClient {
    let greeting = be_block_protocol::Greeting {
        token: Some(token.to_owned()),
    };
    socket
        .send(WsMessage::Binary(hello(BUILD, &greeting)))
        .await
        .unwrap();
    TestClient::welcomed(socket)
        .await
        .expect("the greeting signs in")
}

/// One address holds only so many connections whose greeting has not signed them in, and one
/// that signs in no longer counts among them.
#[tokio::test]
async fn clients_not_signed_in_hold_only_their_share() {
    let harness = Harness::start().await;
    let token = harness.register("signed@example.com", "Signed").await;

    let mut crowd = Vec::new();
    for _ in 0..crate::MAX_UNSIGNED_CONNECTIONS_PER_ADDRESS {
        crowd.push(
            from(&harness, "203.0.113.7")
                .await
                .expect("a connection was refused"),
        );
    }
    assert!(
        from(&harness, "203.0.113.7").await.is_err(),
        "an address held more connections that are not signed in than it may"
    );
    let mut elsewhere = signed_in(
        from(&harness, "198.51.100.1")
            .await
            .expect("another address was refused"),
        &token,
    )
    .await;

    let mut signed = signed_in(crowd.pop().unwrap(), &token).await;
    let another = from(&harness, "203.0.113.7").await;
    assert!(
        another.is_ok(),
        "a connection that signed in still held its place among those that have not"
    );
    signed.round_trip().await;
    elsewhere.round_trip().await;
    drop(crowd);
    harness.stop().await;
}
