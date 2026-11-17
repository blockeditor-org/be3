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
        .call(|peer| {
            client::blocks::set_parent(peer, block, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(matches!(trashed, Ok(())), "{trashed:?}");
    reader
        .heard_change(block, |changed| {
            changed.parent == Some(BlockParent::Detached)
        })
        .await;

    let restored = trasher
        .call(|peer| client::blocks::set_parent(peer, block, BlockParent::Root, KeySet::default()))
        .await;
    assert!(matches!(restored, Ok(())), "{restored:?}");
    let author = Author::new(account);
    let (head, chunks) = author.compose(b"restored and edited\n", 1_000, None);
    trasher
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    let published = trasher
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                head,
                None,
                Vec::new(),
                objects_change(&[], chunks),
                1_000,
                false,
                listing(&[], Vec::new()),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(published.is_ok(), "{published:?}");
    reader.heard_head(block, head).await;

    let trashed = trasher
        .call(|peer| {
            client::blocks::set_parent(peer, block, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(matches!(trashed, Ok(())), "{trashed:?}");
    let response = trasher
        .call(|peer| client::blocks::collect_detached(peer))
        .await;
    let Ok(collected) = response else {
        panic!("collection failed: {response:?}");
    };
    assert!(collected.blocks.contains(&block));
    while reader.graph_change().await != Notice::Roots {}
    let response = reader.try_read_block(block).await;
    assert!(
        refused_as(&response, ErrorCode::BlockNotFound),
        "{response:?}"
    );

    harness.stop().await;
}
