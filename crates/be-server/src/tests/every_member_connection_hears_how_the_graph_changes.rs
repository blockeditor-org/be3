use super::*;

#[tokio::test]
async fn every_member_connection_hears_how_the_graph_changes() {
    let harness = Harness::start().await;
    let mut author = harness.client().await;
    author.register("author@example.com").await;
    let workspace = author.workspace("notes").await;
    let mut other = harness
        .second_connection(&author.token.clone(), workspace)
        .await;

    let folder = author.create_block(BlockParent::Root).await;
    let created = other.graph_change().await;
    assert!(
        matches!(&created, ServerMessage::BlockChanged { block } if block.id == folder),
        "{created:?}"
    );

    let response = author
        .send(|request| ClientMessage::SetMetadata {
            request,
            block: folder,
            metadata: b"sealed name".to_vec(),
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Block { .. }),
        "{response:?}"
    );
    let renamed = other.graph_change().await;
    assert!(
        matches!(&renamed, ServerMessage::BlockChanged { block }
            if block.id == folder && block.metadata == b"sealed name"),
        "{renamed:?}"
    );

    let child = author.create_block(BlockParent::Block(folder)).await;
    other.graph_change().await;
    let response = author
        .send(|request| ClientMessage::DeleteBlock {
            request,
            block: child,
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let detached = other.graph_change().await;
    assert!(
        matches!(&detached, ServerMessage::BlockChanged { block }
            if block.id == child && block.parent == BlockParent::Detached),
        "{detached:?}"
    );

    let listed = other
        .send(|request| ClientMessage::ListBlocks { request })
        .await;
    let ServerMessage::Blocks { blocks, .. } = listed else {
        panic!("listing failed: {listed:?}");
    };
    assert_eq!(blocks.len(), 2);

    let response = author
        .send(|request| ClientMessage::CollectDetached { request })
        .await;
    assert!(
        matches!(response, ServerMessage::Collected { .. }),
        "{response:?}"
    );
    let removed = other.graph_change().await;
    assert!(
        matches!(removed, ServerMessage::BlockRemoved { block } if block == child),
        "{removed:?}"
    );

    harness.stop().await;
}
