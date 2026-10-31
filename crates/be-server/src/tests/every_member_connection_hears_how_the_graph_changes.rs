use super::*;

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
    let created = other.graph_change().await;
    assert!(
        matches!(&created, Notice::BlockChanged { block } if block.id == folder),
        "{created:?}"
    );

    let response = author
        .call(|peer| {
            client::blocks::set_metadata(peer, folder, b"sealed name".to_vec(), Vec::new())
        })
        .await;
    assert!(matches!(response, Ok(_)), "{response:?}");
    let renamed = other.graph_change().await;
    assert!(
        matches!(&renamed, Notice::BlockChanged { block }
            if block.id == folder && block.metadata == b"sealed name"),
        "{renamed:?}"
    );

    other.list(BlockParent::Block(folder)).await;
    let child = author.create_block(BlockParent::Block(folder)).await;
    other.graph_change().await;
    let response = author
        .call(|peer| client::blocks::delete_block(peer, child))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let detached = other.graph_change().await;
    assert!(
        matches!(&detached, Notice::BlockChanged { block }
            if block.id == child && block.parent == Some(BlockParent::Detached)),
        "{detached:?}"
    );

    let blocks = other.list(BlockParent::Detached).await;
    assert_eq!(
        blocks.iter().map(|block| block.id).collect::<Vec<_>>(),
        [child]
    );

    let response = author
        .call(|peer| client::blocks::collect_detached(peer))
        .await;
    assert!(
        matches!(response, Ok(be_protocol::Collected { .. })),
        "{response:?}"
    );
    let removed = other.graph_change().await;
    assert!(
        matches!(removed, Notice::BlockRemoved { block, .. } if block == child),
        "{removed:?}"
    );

    harness.stop().await;
}
