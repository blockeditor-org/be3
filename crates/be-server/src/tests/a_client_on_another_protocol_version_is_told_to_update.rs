use super::*;

#[tokio::test]
async fn a_client_on_another_protocol_version_is_told_to_update() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;

    let response = client
        .send(|request| ClientMessage::Login {
            request,
            version: be_protocol::PROTOCOL_VERSION + 1,
            email: "someone@example.com".into(),
            password: "correct horse battery".into(),
        })
        .await;
    assert!(
        matches!(&response, ServerMessage::Failed { code: ErrorCode::UpdateRequired, message, .. } if message.ends_with("update the server")),
        "{response:?}"
    );

    let mut older = encode_message(&ClientMessage::Authenticate {
        request: 7,
        version: be_protocol::PROTOCOL_VERSION - 1,
        token: String::new(),
    })
    .unwrap();
    older.truncate(3);
    older.push(0xff);
    let response = client.exchange(7, older).await;
    assert!(
        matches!(&response, ServerMessage::Failed { code: ErrorCode::UpdateRequired, message, .. } if message.ends_with("update the app")),
        "{response:?}"
    );

    let mut unreadable = encode_message(&ClientMessage::ListWorkspaces { request: 8 }).unwrap();
    unreadable[0] = 0x7f;
    let response = client.exchange(8, unreadable).await;
    assert!(
        matches!(
            response,
            ServerMessage::Failed {
                request: 8,
                code: ErrorCode::InvalidRequest,
                ..
            }
        ),
        "{response:?}"
    );

    harness.stop().await;
}
