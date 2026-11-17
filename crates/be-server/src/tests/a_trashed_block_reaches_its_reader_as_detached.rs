use super::*;

#[tokio::test]
async fn a_trashed_block_reaches_its_reader_as_detached() {
    let harness = Harness::start().await;
    let mut reader = harness.client().await;
    let account = reader.register("author@example.com").await;
    let organization = reader.organization("notes").await;
    let block = reader.create_block(BlockParent::Root).await;
    reader.follow(block).await;

    let mut trasher = harness
        .second_connection(&reader.token.clone(), organization)
        .await;
    let trashed = trasher
        .move_keyed(block, BlockParent::Detached, KeySet::default())
        .await;
    assert!(matches!(trashed, Ok(())), "{trashed:?}");
    reader
        .heard_change(block, |changed| {
            changed.parent == Some(BlockParent::Detached)
        })
        .await;

    let restored = trasher
        .move_keyed(block, BlockParent::Root, KeySet::default())
        .await;
    assert!(matches!(restored, Ok(())), "{restored:?}");
    let author = Author::new(account);
    let (head, chunks) = author.compose(b"restored and edited\n", 1_000, None);
    trasher
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    let published = trasher
        .call(|peer| {
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                block: block,
                commit: head,
                expected: None,
                parents: Vec::new(),
                held: objects_change(&[], chunks),
                time: 1_000,
                pinned: false,
                references: listing(&[], Vec::new()),
                metadata: Vec::new(),
                objects: Vec::new(),
                journal: None,
            })
        })
        .await;
    assert!(published.is_ok(), "{published:?}");
    reader.heard_head(block, head).await;

    let trashed = trasher
        .move_keyed(block, BlockParent::Detached, KeySet::default())
        .await;
    assert!(matches!(trashed, Ok(())), "{trashed:?}");
    reader
        .heard_change(block, |changed| {
            changed.parent == Some(BlockParent::Detached)
        })
        .await;

    harness.stop().await;
}
