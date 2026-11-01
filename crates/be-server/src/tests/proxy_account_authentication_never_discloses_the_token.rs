use super::*;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[tokio::test]
async fn proxy_account_authentication_never_discloses_the_token() {
    let harness = Harness::start().await;
    let owner = harness.member("proxy@example.com").await;
    for token in [Some(owner.token.as_str()), Some("invalid"), None] {
        let mut request = harness.socket().into_client_request().unwrap();
        if let Some(token) = token {
            request
                .headers_mut()
                .insert("Authorization", format!("Bearer {token}").parse().unwrap());
        }
        let (mut socket, _) = connect_async(request).await.unwrap();
        socket.send(WsMessage::Binary(hello(BUILD))).await.unwrap();
        next_binary(&mut socket).await.unwrap();
        let mut client = TestClient::over(socket);
        let response = client
            .call(|peer| client::account::authenticate(peer, None))
            .await;
        match token {
            Some(token) if token == owner.token => {
                assert!(matches!(response, Ok(Authenticated { token, .. }) if token.is_empty()));
                assert!(matches!(
                    client
                        .call(|peer| client::organizations::list_organizations(peer))
                        .await,
                    Ok(_)
                ));
                let _ = client.call(|peer| client::account::logout(peer)).await;
            }
            _ => assert!(matches!(response, Err(_))),
        }
    }
    let mut client = harness.client().await;
    assert!(matches!(
        client
            .call(|peer| client::account::authenticate(peer, Some(owner.token.clone())))
            .await,
        Err(_)
    ));
    harness.stop().await;
}
