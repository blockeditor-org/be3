use super::*;

#[tokio::test]
async fn a_stale_publish_is_rejected_with_the_current_head() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("one@example.com").await;
    client.organization("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let (base, base_chunks) = author.compose(b"base\n", 1_000, None);
    client
        .upload(&author.store, &author.objects_of(base, &base_chunks))
        .await;
    let _ = client
        .call(|peer| {
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish { block: block, commit: base, expected: None, parents: Vec::new(), held: objects_change(&[], base_chunks.clone()), time: 1_000, pinned: false, references: listing(&[], Vec::new()), metadata: Vec::new(), objects: Vec::new(), journal: None })
        })
        .await;

    let (ours, ours_chunks) = author.compose(b"ours\n", 2_000, Some(base));
    client
        .upload(&author.store, &author.objects_of(ours, &ours_chunks))
        .await;
    let response = client
        .call(|peer| {
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish { block: block, commit: ours, expected: None, parents: Vec::new(), held: objects_change(&[], ours_chunks.clone()), time: 2_000, pinned: false, references: listing(&[], Vec::new()), metadata: Vec::new(), objects: Vec::new(), journal: None })
        })
        .await;
    assert!(
        matches!(response, Ok(be_block_protocol::PublishOutcome::Rejected(head)) if head == Some(base)),
        "{response:?}"
    );
    assert_eq!(client.read_block(block).await.head, Some(base));

    let response = client
        .call(|peer| {
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish { block: block, commit: ours, expected: Some(base), parents: Some(base).into_iter().collect(), held: objects_change(&base_chunks, ours_chunks), time: 2_000, pinned: false, references: listing(&[], Vec::new()), metadata: Vec::new(), objects: Vec::new(), journal: None })
        })
        .await;
    assert!(
        matches!(
            response,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{response:?}"
    );

    harness.stop().await;
}
