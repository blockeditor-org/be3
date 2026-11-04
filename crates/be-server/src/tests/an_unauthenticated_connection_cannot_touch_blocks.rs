use super::*;

#[tokio::test]
async fn an_unauthenticated_connection_cannot_touch_blocks() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("private").await;
    let block = owner.create_block(BlockParent::Root).await;

    let mut stranger = harness.client().await;
    let response = stranger
        .call(|peer| client::blocks::read_block(peer, block))
        .await;
    assert!(
        matches!(response, Err(Error { code, .. }) if code == ErrorCode::NotAuthenticated),
        "{response:?}"
    );

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
        .call(|peer| client::blocks::read_block(peer, block))
        .await;
    assert!(
        matches!(response, Err(Error { code, .. }) if code == ErrorCode::NotAuthenticated),
        "{response:?}"
    );

    harness.stop().await;
}
