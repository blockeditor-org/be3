use super::*;

#[tokio::test]
async fn a_watcher_is_told_when_a_block_is_detached() {
    let harness = Harness::start().await;
    let mut watcher = harness.client().await;
    watcher.register("author@example.com").await;
    let organization = watcher.organization("notes").await;
    let block = watcher.create_block(BlockParent::Root).await;
    watcher.watch(block).await;

    let mut trasher = harness
        .second_connection(&watcher.token.clone(), organization)
        .await;
    let response = trasher
        .call(|peer| {
            client::blocks::set_parent(peer, block, BlockParent::Detached, KeySet::default())
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");

    let notification = watcher.notification().await;
    assert!(
        matches!(notification, Notice::BlockDeleted { block: gone } if gone == block),
        "{notification:?}"
    );

    harness.stop().await;
}
