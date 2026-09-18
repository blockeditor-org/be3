use super::*;

#[tokio::test]
async fn a_stale_publish_is_rejected_with_the_current_head() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("one@example.com").await;
    client.workspace("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let (base, base_chunks) = author.compose(b"base\n", 1_000, None);
    client
        .upload(&author.store, &author.objects_of(base, &base_chunks))
        .await;
    client
        .send(|request| ClientMessage::Publish {
            request,
            block,
            commit: base,
            expected: None,
            chunks: base_chunks,
            time: 1_000,
            pinned: false,
            references_added: Vec::new(),
            references_removed: Vec::new(),
        })
        .await;

    let (ours, ours_chunks) = author.compose(b"ours\n", 2_000, Some(base));
    client
        .upload(&author.store, &author.objects_of(ours, &ours_chunks))
        .await;
    let response = client
        .send(|request| ClientMessage::Publish {
            request,
            block,
            commit: ours,
            expected: None,
            chunks: ours_chunks.clone(),
            time: 2_000,
            pinned: false,
            references_added: Vec::new(),
            references_removed: Vec::new(),
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Rejected { head, .. } if head == Some(base)),
        "{response:?}"
    );
    assert_eq!(client.read_block(block).await.head, Some(base));

    let response = client
        .send(|request| ClientMessage::Publish {
            request,
            block,
            commit: ours,
            expected: Some(base),
            chunks: ours_chunks,
            time: 2_000,
            pinned: false,
            references_added: Vec::new(),
            references_removed: Vec::new(),
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Published { .. }),
        "{response:?}"
    );

    harness.stop().await;
}
