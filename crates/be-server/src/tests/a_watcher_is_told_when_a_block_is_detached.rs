use super::*;

#[tokio::test]
async fn a_watcher_is_told_when_a_block_is_detached() {
    let harness = Harness::start().await;
    let mut watcher = harness.client().await;
    let account = watcher.register("author@example.com").await;
    let organization = watcher.organization("notes").await;
    let block = watcher.create_block(BlockParent::Root).await;
    watcher.watch(block).await;

    let mut trasher = harness
        .second_connection(&watcher.token.clone(), organization)
        .await;
    for parent in [BlockParent::Detached, BlockParent::Root] {
        let response = trasher
            .call(|peer| client::blocks::set_parent(peer, block, parent, KeySet::default()))
            .await;
        assert!(matches!(response, Ok(())), "{response:?}");
    }

    let author = Author::new(account);
    let (head, chunks) = author.compose(b"restored and edited\n", 1_000, None);
    trasher
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    let _ = trasher
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
    let notification = watcher.notification().await;
    assert!(
        matches!(
            notification,
            Notice::HeadChanged { block: changed, head: moved, .. }
                if changed == block && moved == head
        ),
        "a block trashed and restored is still watched: {notification:?}"
    );

    let response = trasher
        .call(|peer| {
            client::blocks::set_parent(peer, block, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let response = trasher
        .call(|peer| client::blocks::collect_detached(peer))
        .await;
    let Ok(collected) = response else {
        panic!("collection failed: {response:?}");
    };
    assert!(collected.blocks.contains(&block));
    let notification = watcher.notification().await;
    assert!(
        matches!(notification, Notice::BlockDeleted { block: gone } if gone == block),
        "{notification:?}"
    );

    harness.stop().await;
}
