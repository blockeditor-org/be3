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
        .call(|peer| {
            peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::Publish {
                block,
                expected: None,
                commit: None,
                references: be_block_protocol::ReferenceChange::between(&[], &Vec::new()),
                metadata: vec![1, 2, 3],
                journal: None,
            })
        })
        .await;
    let Ok(renamed) = response else {
        panic!("set metadata failed: {response:?}");
    };
    assert!(renamed.version > created);

    let response = owner
        .move_keyed(block, BlockParent::Block(folder), KeySet::default())
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let moved = owner.read_block(block).await;
    assert!(moved.version > renamed.version);
    assert_eq!(moved.metadata, vec![1, 2, 3]);

    harness.stop().await;
}
