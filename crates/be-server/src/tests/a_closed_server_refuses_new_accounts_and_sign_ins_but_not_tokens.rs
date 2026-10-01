use super::*;

#[tokio::test]
async fn a_closed_server_refuses_new_accounts_and_sign_ins_but_not_tokens() {
    let directory = std::env::temp_dir().join(format!("be-server-test-{}", Uuid::new_v4()));
    let (account, workspace) = add_account(
        &directory,
        "owner@example.com",
        "Owner",
        "correct horse battery",
        "notes",
    )
    .await
    .unwrap();
    let token = ServerStore::open(&directory)
        .unwrap()
        .issue_session(account)
        .await
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown, receiver) = oneshot::channel::<()>();
    let data_dir = directory.clone();
    let server = tokio::spawn(async move {
        serve_with_config(listener, data_dir, ServerConfig::default(), receiver).await
    });
    let (socket, _) = connect_async(format!("ws://{address}")).await.unwrap();
    let mut client = TestClient {
        socket,
        next: 0,
        notifications: Vec::new(),
        token: String::new(),
    };

    let refused = client
        .send(|request| ClientMessage::Register {
            request,
            email: "stranger@example.com".into(),
            display_name: "Stranger".into(),
            password: "correct horse battery".into(),
        })
        .await;
    assert!(
        matches!(
            refused,
            ServerMessage::Failed {
                code: ErrorCode::RegistrationDisabled,
                ..
            }
        ),
        "{refused:?}"
    );
    let refused = client
        .send(|request| ClientMessage::Login {
            request,
            email: "owner@example.com".into(),
            password: "correct horse battery".into(),
        })
        .await;
    assert!(
        matches!(
            refused,
            ServerMessage::Failed {
                code: ErrorCode::LoginDisabled,
                ..
            }
        ),
        "the right password signed in while sign-ins were off: {refused:?}"
    );
    let resumed = client
        .send(|request| ClientMessage::Authenticate {
            request,
            token: token.clone(),
        })
        .await;
    assert!(
        matches!(resumed, ServerMessage::Authenticated { .. }),
        "a device already signed in was turned away: {resumed:?}"
    );
    client.open(workspace).await;

    let _ = shutdown.send(());
    server.await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&directory);
}
