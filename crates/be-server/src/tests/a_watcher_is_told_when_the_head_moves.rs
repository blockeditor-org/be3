use super::*;

#[tokio::test]
async fn a_watcher_is_told_when_the_head_moves() {
    let harness = Harness::start().await;
    let mut watcher = harness.client().await;
    let account = watcher.register("author@example.com").await;
    let workspace = watcher.workspace("notes").await;
    let block = watcher.create_block(BlockParent::Root).await;
    let response = watcher
        .send(|request| ClientMessage::Watch { request, block })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");

    let mut publisher = harness
        .second_connection(&watcher.token.clone(), workspace)
        .await;
    let author = Author::new(account);
    let (head, chunks) = author.compose(b"published elsewhere\n", 1_000, None);
    publisher
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    publisher
        .send(|request| ClientMessage::Publish {
            request,
            block,
            commit: head,
            expected: None,
            chunks,
            time: 1_000,
            pinned: false,
            references_added: Vec::new(),
            references_removed: Vec::new(),
        })
        .await;

    let notification = watcher.notification().await;
    assert!(
        matches!(
            notification,
            ServerMessage::HeadChanged { block: changed, head: moved, author: by }
                if changed == block && moved == head && by == account
        ),
        "{notification:?}"
    );

    harness.stop().await;
}
