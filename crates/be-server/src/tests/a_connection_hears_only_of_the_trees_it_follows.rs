use super::*;

async fn rename(client: &mut TestClient, block: Uuid, name: &[u8]) {
    let response = client
        .call(|peer| client::blocks::set_metadata(peer, block, name.to_vec(), Vec::new()))
        .await;
    assert!(matches!(response, Ok(_)), "{response:?}");
}

/// The entry point the next `tree-advanced` names, read to its end so the next change is told.
async fn advanced(client: &mut TestClient) -> Uuid {
    loop {
        if let Notice::TreeAdvanced { entry, .. } = client.graph_change().await {
            client.read_changes(entry).await;
            return entry;
        }
    }
}

#[tokio::test]
async fn a_connection_hears_only_of_the_trees_it_follows() {
    let harness = Harness::start().await;
    let mut author = harness.client().await;
    author.register("author@example.com").await;
    let organization = author.organization("notes").await;
    let mut other = harness
        .second_connection(&author.token.clone(), organization)
        .await;
    let followed = author.create_block(BlockParent::Root).await;
    let unfollowed = author.create_block(BlockParent::Root).await;
    assert_eq!(other.follow(followed).await, followed);

    author.create_block(BlockParent::Block(unfollowed)).await;
    rename(&mut author, unfollowed, b"unheard").await;
    let inside = author.create_block(BlockParent::Block(followed)).await;
    assert_eq!(
        advanced(&mut other).await,
        followed,
        "a change in a tree nobody follows is not told"
    );
    let (summaries, _) = other.read_changes(followed).await;
    assert!(summaries.is_empty(), "read to its end already");

    let response = author
        .call(|peer| {
            client::blocks::set_parent(peer, inside, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let entry = loop {
        if let Notice::TreeAdvanced { entry, .. } = other.graph_change().await {
            break entry;
        }
    };
    assert_eq!(entry, followed);
    let (_, removed) = other.read_changes(followed).await;
    assert_eq!(removed, [inside], "a block leaving a followed tree is named");

    let response = other
        .call(|peer| client::blocks::unfollow(peer, followed))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    rename(&mut author, followed, b"unheard too").await;
    other.follow(unfollowed).await;
    rename(&mut author, unfollowed, b"heard").await;
    assert_eq!(
        advanced(&mut other).await,
        unfollowed,
        "nothing in a tree is told once it is unfollowed"
    );

    harness.stop().await;
}
