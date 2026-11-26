use super::*;

/// A write's ops apply in one transaction: the second refused, the first, which alone would
/// have been taken, is kept neither in the database nor in the graph the server holds.
#[tokio::test]
async fn a_write_whose_second_op_is_refused_keeps_nothing_of_its_first() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("team").await;
    let (block, create) = owner.new_block_op(BlockParent::Root).await;
    let refused = Op::Publish {
        block: Uuid::new_v4(),
        expected: None,
        commit: None,
        references: be_block_protocol::ReferenceChange::between(&[], &Vec::new()),
        metadata: b"a block that is not there".to_vec(),
        journal: None,
    };

    let mut ops = create.clone();
    ops.push(refused);
    let written = client::server::write(&owner.peer, Some(organization), ops)
        .await
        .expect("the connection stayed open");
    let Err(WriteError { index, error }) = written else {
        panic!("a write with a refused op was taken: {written:?}");
    };
    assert_eq!(index, 2, "the refusal names the op refused: {error:?}");

    let read = owner.try_read_block(block).await;
    assert!(
        refused_as(&read, ErrorCode::BlockNotFound),
        "the first op's block was kept: {read:?}"
    );
    assert!(
        owner
            .list(BlockParent::Root)
            .await
            .iter()
            .all(|listed| listed.id != block),
        "the first op's block is listed"
    );

    let made = owner
        .call(|peer| peer.write_all::<BlockSummary>(create))
        .await;
    assert!(
        made.is_ok(),
        "the same op alone was refused, as if a graph change were left behind: {made:?}"
    );
    assert_eq!(owner.read_block(block).await.id, block);

    harness.stop().await;
}
