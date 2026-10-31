use super::*;

#[tokio::test]
async fn a_watcher_is_told_when_the_head_moves() {
    let harness = Harness::start().await;
    let mut watcher = harness.client().await;
    let account = watcher.register("author@example.com").await;
    let organization = watcher.organization("notes").await;
    let block = watcher.create_block(BlockParent::Root).await;
    watcher.watch(block).await;

    let mut publisher = harness
        .second_connection(&watcher.token.clone(), organization)
        .await;
    let author = Author::new(account);
    let (head, chunks) = author.compose(b"published elsewhere\n", 1_000, None);
    publisher
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    let _ = publisher
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                head,
                None,
                Vec::new(),
                chunks,
                1_000,
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;

    let notification = watcher.notification().await;
    assert!(
        matches!(
            notification,
            Notice::HeadChanged { block: changed, head: moved, author: by }
                if changed == block && moved == head && by == account
        ),
        "{notification:?}"
    );

    harness.stop().await;
}
