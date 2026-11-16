use super::*;

/// The server names a block's head; a commit sealed in another block is refused as its head and
/// as its metadata rather than shown as this block's and written back under its key.
#[tokio::test]
async fn another_blocks_commit_served_as_a_head_is_refused() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let (open, secret) = open_and_secret(&owner).await;
    let open_head = owner.summary(open).await.unwrap().head.expect("a head");
    let secret_head = owner.summary(secret).await.unwrap().head.expect("a head");

    assert!(owner.load_head(open, open_head).await.is_ok());
    assert!(owner.metadata_of(secret, secret_head).await.is_ok());
    let loaded = owner.load_head(secret, open_head).await;
    assert!(
        matches!(loaded, Err(ClientError::Forged(block)) if block == secret),
        "the open note's commit is not the secret note's head: {loaded:?}"
    );
    let metadata = owner.metadata_of(secret, open_head).await;
    assert!(
        matches!(metadata, Err(ClientError::Forged(block)) if block == secret),
        "nor its metadata: {metadata:?}"
    );
}
