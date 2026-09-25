use super::*;

#[tokio::test]
async fn every_graph_change_advances_a_blocks_version() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.workspace("versions").await;
    let folder = owner.create_block(BlockParent::Root).await;
    let block = owner.create_block(BlockParent::Root).await;
    let created = owner.read_block(block).await.version;

    let response = owner
        .send(|request| ClientMessage::SetMetadata {
            request,
            block,
            metadata: vec![1, 2, 3],
        })
        .await;
    let ServerMessage::Block { block: renamed, .. } = response else {
        panic!("set metadata failed: {response:?}");
    };
    assert!(renamed.version > created);

    let response = owner
        .send(|request| ClientMessage::SetParent {
            request,
            block,
            parent: BlockParent::Block(folder),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    let moved = owner.read_block(block).await;
    assert!(moved.version > renamed.version);
    assert_eq!(moved.metadata, vec![1, 2, 3]);

    harness.stop().await;
}
