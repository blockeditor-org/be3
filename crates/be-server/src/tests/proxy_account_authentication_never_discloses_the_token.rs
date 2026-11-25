use super::*;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

/// A greeting with no token signs in with the request's Authorization header, which a proxy
/// injects, and the server's greeting never tells the token back.
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
        socket
            .send(WsMessage::Binary(one_shot_hello()))
            .await
            .unwrap();
        let signed_in = TestClient::welcomed(socket).await;
        match token {
            Some(token) if token == owner.token => {
                let mut client = signed_in.expect("the header signs the greeting in");
                assert!(client.token.is_empty(), "the server told the token back");
                assert!(
                    client
                        .call(|peer| peer.read::<Vec<be_block_protocol::Organization>>(
                            be_block_protocol::Query::Organizations
                        ))
                        .await
                        .is_ok()
                );
                let _ = client
                    .call(|peer| peer.write::<()>(be_block_protocol::Op::RevokeToken(None)))
                    .await;
            }
            _ => assert!(signed_in.is_err()),
        }
    }
    assert!(
        harness.sign_in(&owner.token).await.is_err(),
        "a token logged out through the header still signs in"
    );
    harness.stop().await;
}
