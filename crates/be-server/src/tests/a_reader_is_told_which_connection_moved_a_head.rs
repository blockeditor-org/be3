use super::*;

#[tokio::test]
async fn a_reader_is_told_which_connection_moved_a_head() {
    let harness = Harness::start().await;
    let mut reader = harness.client().await;
    let account = reader.register("author@example.com").await;
    let organization = reader.organization("notes").await;
    let block = reader.create_block(BlockParent::Root).await;
    reader.follow(block).await;

    let mut publisher = harness
        .second_connection(&reader.token.clone(), organization)
        .await;
    publisher.follow(block).await;
    let author = Author::new(account);
    let (head, chunks) = author.compose(b"published elsewhere\n", 1_000, None);
    publisher
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    let published = publisher
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
    assert!(published.is_ok(), "{published:?}");

    let by = reader.heard_head(block, head).await;
    assert_eq!(
        Some(by),
        publisher.client,
        "the reader is told who published"
    );
    assert_ne!(publisher.client, reader.client);
    let by = publisher.heard_head(block, head).await;
    assert_eq!(
        Some(by),
        publisher.client,
        "the publisher hears its own publish as its own"
    );

    harness.stop().await;
}
