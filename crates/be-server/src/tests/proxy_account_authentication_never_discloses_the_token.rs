use super::*;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[tokio::test]
async fn proxy_account_authentication_never_discloses_the_token() {
    let harness = Harness::start().await;
    let owner = harness.member("proxy@example.com").await;
    for token in [Some(owner.token.as_str()), Some("invalid"), None] {
        let mut request = harness.url.as_str().into_client_request().unwrap();
        if let Some(token) = token {
            request
                .headers_mut()
                .insert("Authorization", format!("Bearer {token}").parse().unwrap());
        }
        let (mut socket, _) = connect_async(request).await.unwrap();
        socket.send(WsMessage::Binary(hello(BUILD))).await.unwrap();
        next_binary(&mut socket).await.unwrap();
        let mut client = TestClient {
            directory: None,
            socket,
            next: 0,
            notifications: Vec::new(),
            token: String::new(),
        };
        let response = client
            .send(|request| ClientMessage::AuthenticateHeader { request })
            .await;
        match token {
            Some(token) if token == owner.token => {
                assert!(
                    matches!(response, ServerMessage::Authenticated { token, .. } if token.is_empty())
                );
                assert!(matches!(
                    client
                        .send(|request| ClientMessage::ListOrganizations { request })
                        .await,
                    ServerMessage::Organizations { .. }
                ));
                client
                    .send(|request| ClientMessage::Logout { request })
                    .await;
            }
            _ => assert!(matches!(response, ServerMessage::Failed { .. })),
        }
    }
    let mut client = harness.client().await;
    assert!(matches!(
        client
            .send(|request| ClientMessage::Authenticate {
                request,
                token: owner.token.clone()
            })
            .await,
        ServerMessage::Failed { .. }
    ));
    harness.stop().await;
}
