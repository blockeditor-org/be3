use super::*;

#[tokio::test]
async fn requests_sent_before_a_close_are_still_carried_out() {
    let harness = Harness::start().await;
    let mut reader = harness.client().await;
    let account = reader.register("author@example.com").await;
    let organization = reader.organization("notes").await;
    let blocks = [
        reader.create_block(BlockParent::Root).await,
        reader.create_block(BlockParent::Root).await,
        reader.create_block(BlockParent::Root).await,
    ];
    for block in blocks {
        reader.follow(block).await;
    }

    let mut publisher = harness
        .second_connection(&reader.token.clone(), organization)
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
            publisher.peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish { block: block, commit: head, expected: None, parents: Vec::new(), held: objects_change(&[], chunks), time: 1_000, pinned: false, references: listing(&[], Vec::new()), metadata: Vec::new(), objects: Vec::new(), journal: None })
        })
        .collect();
    publisher.close().await;
    drop(sent);

    let mut moved = Vec::new();
    while moved.len() < heads.len() {
        if let Notice::TreeAdvanced { entry, .. } = reader.graph_change().await {
            let (summaries, _) = reader.read_changes(entry).await;
            moved.extend(
                summaries
                    .into_iter()
                    .filter_map(|block| Some((block.id, block.head?))),
            );
        }
    }
    let expected: Vec<_> = heads
        .iter()
        .map(|(block, head, _)| (*block, *head))
        .collect();
    assert_eq!(moved, expected);

    harness.stop().await;
}
