use super::*;

/// A block is made with the data key it names stored in the same write: `[create-block]` alone,
/// or beside a key of another block, is refused at the write's end, and nothing of it is kept.
#[tokio::test]
async fn a_block_made_with_no_key_in_its_write_is_refused() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.personal().await;
    let (block, made) = owner.new_block_op(BlockParent::Root).await;
    let create = made[0].clone();

    let alone = client::server::write(&owner.peer, Some(organization), vec![create.clone()])
        .await
        .expect("the connection stayed open");
    let Err(WriteError { index, error }) = alone else {
        panic!("a block made with no key was taken: {alone:?}");
    };
    assert_eq!(
        (index, error.code),
        (0, ErrorCode::InvalidRequest),
        "{error:?}"
    );

    let other = owner.create_block(BlockParent::Root).await;
    let elsewhere = Uuid::new_v4();
    let beside = Op::PutKeys(PutKeys::Rows(KeySet {
        keys: vec![row(elsewhere, other, KeyKind::Data)],
        wrapped: Vec::new(),
    }));
    let written = client::server::write(&owner.peer, Some(organization), vec![create, beside])
        .await
        .expect("the connection stayed open");
    assert!(
        matches!(written, Err(WriteError { index: 0, .. })),
        "a block made beside another block's key was taken: {written:?}"
    );
    let read = owner.try_read_block(block).await;
    assert!(
        refused_as(&read, ErrorCode::BlockNotFound),
        "a block refused was kept: {read:?}"
    );
    assert!(
        owner
            .keys(Some(vec![other]))
            .await
            .keys
            .iter()
            .all(|key| key.id != elsewhere),
        "the key written beside a refused block was kept"
    );

    let made = owner
        .call(|peer| peer.write_all::<BlockSummary>(made))
        .await;
    assert!(made.is_ok(), "{made:?}");

    harness.stop().await;
}
