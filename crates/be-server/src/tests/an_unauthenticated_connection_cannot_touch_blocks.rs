use super::*;

#[tokio::test]
async fn an_unauthenticated_connection_cannot_touch_blocks() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("private").await;
    let block = owner.create_block(BlockParent::Root).await;

    let mut stranger = harness.client().await;
    let response = client::blocks::list_blocks(&stranger.peer, Listing::Block(block)).await;
    assert!(
        !matches!(response, Ok(Ok(_))),
        "a connection read a block before signing in: {response:?}"
    );
    stranger.ended().await;

    let mut stranger = harness.client().await;
    let response = stranger
        .call(|peer| client::account::authenticate(peer, Some("not a token".into())))
        .await;
    assert!(
        matches!(response, Err(Error { code, .. }) if code == ErrorCode::InvalidCredentials),
        "{response:?}"
    );

    stranger.register("stranger@example.com").await;
    let response = stranger
        .call(|peer| client::organizations::open_organization(peer, organization))
        .await;
    assert!(
        matches!(response, Err(Error { code, .. }) if code == ErrorCode::PermissionDenied),
        "{response:?}"
    );

    let response = stranger
        .try_read(block).await;
    assert!(
        matches!(response, Err(Error { code, .. }) if code == ErrorCode::NotAuthenticated),
        "{response:?}"
    );

    harness.stop().await;
}
