use super::*;

#[tokio::test]
async fn every_graph_change_advances_a_blocks_version() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    owner.organization("versions").await;
    let folder = owner.create_block(BlockParent::Root).await;
    let block = owner.create_block(BlockParent::Root).await;
    let created = owner.read_block(block).await.version;

    let response = owner
        .call(|peer| client::blocks::set_metadata(peer, block, vec![1, 2, 3], Vec::new()))
        .await;
    let Ok(renamed) = response else {
        panic!("set metadata failed: {response:?}");
    };
    assert!(renamed.version > created);

    let response = owner
        .call(|peer| {
            client::blocks::set_parent(peer, block, BlockParent::Block(folder), KeySet::default())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let moved = owner.read_block(block).await;
    assert!(moved.version > renamed.version);
    assert_eq!(moved.metadata, vec![1, 2, 3]);

    harness.stop().await;
}
