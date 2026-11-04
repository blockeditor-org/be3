use super::*;

#[tokio::test]
async fn requests_sent_before_a_close_are_still_carried_out() {
    let harness = Harness::start().await;
    let mut watcher = harness.client().await;
    let account = watcher.register("author@example.com").await;
    let organization = watcher.organization("notes").await;
    let blocks = [
        watcher.create_block(BlockParent::Root).await,
        watcher.create_block(BlockParent::Root).await,
        watcher.create_block(BlockParent::Root).await,
    ];
    for block in blocks {
        watcher.watch(block).await;
    }

    let mut publisher = harness
        .second_connection(&watcher.token.clone(), organization)
        .await;
    let author = Author::new(account);
    let mut heads = Vec::new();
    for (index, block) in blocks.into_iter().enumerate() {
        let (head, chunks) = author.compose(format!("draft {index}\n").as_bytes(), 1_000, None);
        publisher
            .upload(&author.store, &author.objects_of(head, &chunks))
            .await;
        heads.push((block, head, chunks));
    }
    let sent: Vec<_> = heads
        .iter()
        .cloned()
        .map(|(block, head, chunks)| {
            client::blocks::publish(
                &publisher.peer,
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
        .collect();
    publisher.close().await;
    drop(sent);

    let mut moved = Vec::new();
    while moved.len() < heads.len() {
        if let Notice::HeadChanged { block, head, .. } = watcher.notification().await {
            moved.push((block, head));
        }
    }
    let expected: Vec<_> = heads
        .iter()
        .map(|(block, head, _)| (*block, *head))
        .collect();
    assert_eq!(moved, expected);

    harness.stop().await;
}
