use super::*;

#[tokio::test]
async fn a_closed_server_refuses_new_accounts_and_sign_ins_but_not_tokens() {
    let directory = std::env::temp_dir().join(format!("be-server-test-{}", Uuid::new_v4()));
    let (account, organization) = fixture_account(
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
    build_id::install(BUILD);
    let mut client = TestClient::connect(&format!("ws://{address}")).await;

    let resumed = client
        .call(|peer| client::account::authenticate(peer, token.clone()))
        .await;
    assert!(
        matches!(resumed, Ok(_)),
        "a device already signed in was turned away: {resumed:?}"
    );
    client.open(organization).await;

    let _ = shutdown.send(());
    server.await.unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&directory);
}
