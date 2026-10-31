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
            client::blocks::publish(
                peer,
                block,
                base,
                None,
                Vec::new(),
                base_chunks,
                1_000,
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;

    let (ours, ours_chunks) = author.compose(b"ours\n", 2_000, Some(base));
    client
        .upload(&author.store, &author.objects_of(ours, &ours_chunks))
        .await;
    let response = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                ours,
                None,
                Vec::new(),
                ours_chunks.clone(),
                2_000,
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(
        matches!(response, Ok(be_protocol::PublishOutcome::Rejected(head)) if head == Some(base)),
        "{response:?}"
    );
    assert_eq!(client.read_block(block).await.head, Some(base));

    let response = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                ours,
                Some(base),
                Some(base).into_iter().collect(),
                ours_chunks,
                2_000,
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(
        matches!(response, Ok(be_protocol::PublishOutcome::Published(_))),
        "{response:?}"
    );

    harness.stop().await;
}
