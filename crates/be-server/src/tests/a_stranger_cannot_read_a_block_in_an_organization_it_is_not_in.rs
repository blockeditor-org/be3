use super::*;

#[tokio::test]
async fn a_stranger_cannot_read_a_block_in_an_organization_it_is_not_in() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("private").await;
    let block = owner.create_block(BlockParent::Root).await;

    let mut stranger = harness.member("stranger@example.com").await;
    let response = stranger.try_read_block(block).await;
    assert!(
        matches!(response, Err(Error { code, .. }) if code == ErrorCode::InvalidRequest),
        "a read of a block naming no organization was answered: {response:?}"
    );
    stranger.peer.organization = Some(organization);
    let response = stranger.try_read_block(block).await;
    assert!(
        matches!(response, Err(Error { code, .. }) if code == ErrorCode::PermissionDenied),
        "{response:?}"
    );

    harness.stop().await;
}
