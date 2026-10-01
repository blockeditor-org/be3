use super::*;

#[tokio::test]
async fn pruning_history_keeps_pinned_commits() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("keeper@example.com").await;
    client.workspace("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let (first, first_chunks) = author.compose(b"a bookmarked draft\n", 1_000, None);
    let (second, second_chunks) = author.compose(b"a later draft\n", 2_000, Some(first));
    client
        .upload(&author.store, &author.objects_of(first, &first_chunks))
        .await;
    client
        .send(|request| ClientMessage::Publish {
            request,
            block,
            commit: first,
            expected: None,
            chunks: first_chunks.clone(),
            time: 1_000,
            pinned: true,
            references_added: Vec::new(),
            references_removed: Vec::new(),
        })
        .await;
    client
        .publish(&author, block, second, second_chunks, Some(first), 2_000)
        .await;

    let response = client
        .send(|request| ClientMessage::PruneHistory {
            request,
            block,
            drop: vec![first, second],
        })
        .await;
    let ServerMessage::Collected { objects, .. } = response else {
        panic!("pruning failed: {response:?}");
    };
    assert_eq!(objects, 0, "pruning freed a pinned commit or the head");

    let kept: Vec<CommitId> = client
        .history(block)
        .await
        .into_iter()
        .map(|entry| entry.commit)
        .collect();
    assert!(kept.contains(&first), "the pinned commit was pruned");
    assert!(kept.contains(&second), "the head was pruned");

    let response = client
        .send(|request| ClientMessage::GetObject {
            request,
            hash: first.hash(),
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Object { bytes: Some(_), .. }),
        "the pinned commit's object was deleted"
    );

    harness.stop().await;
}
