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

    let harness = Harness::serve(directory, ServerConfig::default()).await;
    let mut client = harness.client().await;

    let resumed = client
        .call(|peer| client::account::authenticate(peer, token.clone()))
        .await;
    assert!(
        matches!(resumed, Ok(_)),
        "a device already signed in was turned away: {resumed:?}"
    );
    client.open(organization).await;

    harness.stop().await;
}
