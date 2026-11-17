use super::*;

/// Waits for the next `tree-advanced`, skipping what else is told.
async fn advanced(client: &mut TestClient) -> Uuid {
    loop {
        if let Notice::TreeAdvanced { entry, .. } = client.graph_change().await {
            return entry;
        }
    }
}

/// Waits for the next `roots`, skipping what else is told.
async fn roots(client: &mut TestClient) {
    while client.graph_change().await != Notice::Roots {}
}

#[tokio::test]
async fn every_member_connection_hears_how_the_graph_changes() {
    let harness = Harness::start().await;
    let mut author = harness.client().await;
    author.register("author@example.com").await;
    let organization = author.organization("notes").await;
    let mut other = harness
        .second_connection(&author.token.clone(), organization)
        .await;
    other.list(BlockParent::Root).await;

    let folder = author.create_block(BlockParent::Root).await;
    roots(&mut other).await;
    assert!(
        other
            .list(BlockParent::Root)
            .await
            .iter()
            .any(|block| block.id == folder),
        "a new top-level block is told as the roots changing"
    );
    other.follow(folder).await;

    let response = author
        .call(|peer| {
            client::blocks::set_metadata(peer, folder, b"sealed name".to_vec(), Vec::new())
        })
        .await;
    assert!(matches!(response, Ok(_)), "{response:?}");
    assert_eq!(advanced(&mut other).await, folder);
    let (renamed, _) = other.read_changes(folder).await;
    assert!(
        renamed
            .iter()
            .any(|block| block.id == folder && block.metadata == b"sealed name"),
        "{renamed:?}"
    );

    let child = author.create_block(BlockParent::Block(folder)).await;
    assert_eq!(advanced(&mut other).await, folder);
    let (made, _) = other.read_changes(folder).await;
    assert!(made.iter().any(|block| block.id == child), "{made:?}");
    let response = author
        .move_keyed(child, BlockParent::Detached, KeySet::default())
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert_eq!(advanced(&mut other).await, folder);
    let (_, left) = other.read_changes(folder).await;
    assert_eq!(left, [child], "a block trashed leaves its tree");

    let blocks = other.list(BlockParent::Detached).await;
    assert_eq!(
        blocks.iter().map(|block| block.id).collect::<Vec<_>>(),
        [child]
    );

    let response = author
        .call(|peer| client::blocks::collect_detached(peer))
        .await;
    assert!(
        matches!(response, Ok(be_block_protocol::Collected { .. })),
        "{response:?}"
    );
    roots(&mut other).await;
    assert!(other.list(BlockParent::Detached).await.is_empty());

    harness.stop().await;
}
