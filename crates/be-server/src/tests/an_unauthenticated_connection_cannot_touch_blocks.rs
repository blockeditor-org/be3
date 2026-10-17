use super::*;

#[tokio::test]
async fn an_unauthenticated_connection_cannot_touch_blocks() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("private").await;
    let block = owner.create_block(BlockParent::Root).await;

    let mut stranger = harness.client().await;
    let response = stranger
        .send(|request| ClientMessage::ReadBlock { request, block })
        .await;
    assert!(
        matches!(response, ServerMessage::Failed { code, .. } if code == ErrorCode::NotAuthenticated),
        "{response:?}"
    );

    let response = stranger
        .send(|request| ClientMessage::Authenticate {
            request,
            token: "not a token".into(),
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Failed { code, .. } if code == ErrorCode::InvalidCredentials),
        "{response:?}"
    );

    stranger.register("stranger@example.com").await;
    let response = stranger
        .send(|request| ClientMessage::OpenOrganization {
            request,
            organization,
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Failed { code, .. } if code == ErrorCode::PermissionDenied),
        "{response:?}"
    );

    let response = stranger
        .send(|request| ClientMessage::ReadBlock { request, block })
        .await;
    assert!(
        matches!(response, ServerMessage::Failed { code, .. } if code == ErrorCode::NotAuthenticated),
        "{response:?}"
    );

    harness.stop().await;
}
