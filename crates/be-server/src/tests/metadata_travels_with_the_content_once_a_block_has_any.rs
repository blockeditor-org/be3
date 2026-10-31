use super::*;

#[tokio::test]
async fn metadata_travels_with_the_content_once_a_block_has_any() {
    let harness = Harness::start().await;
    let mut author_connection = harness.client().await;
    let account = author_connection.register("author@example.com").await;
    let organization = author_connection.organization("notes").await;
    let block = author_connection.create_block(BlockParent::Root).await;
    let mut other = harness
        .second_connection(&author_connection.token.clone(), organization)
        .await;
    other.list(BlockParent::Root).await;

    let response = author_connection
        .call(|peer| {
            client::blocks::set_metadata(peer, block, b"sealed before content".to_vec(), Vec::new())
        })
        .await;
    assert!(matches!(response, Ok(_)), "{response:?}");

    let author = Author::new(account);
    let (head, chunks) = author.compose(b"first\n", 1_000, None);
    author_connection
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    let response = author_connection
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
                b"sealed with content".to_vec(),
                Vec::new(),
            )
        })
        .await;
    assert!(
        matches!(response, Ok(be_protocol::PublishOutcome::Published(_))),
        "{response:?}"
    );
    assert_eq!(
        author_connection.read_block(block).await.metadata,
        b"sealed with content"
    );
    let heard = loop {
        if let Notice::BlockChanged { block: changed } = other.graph_change().await
            && changed.id == block
            && changed.head == Some(head)
        {
            break changed;
        }
    };
    assert_eq!(heard.metadata, b"sealed with content");

    let response = author_connection
        .call(|peer| {
            client::blocks::set_metadata(peer, block, b"sealed beside content".to_vec(), Vec::new())
        })
        .await;
    assert!(
        matches!(
            response,
            Err(Error {
                code: ErrorCode::BlockHasContent,
                ..
            })
        ),
        "{response:?}"
    );
    assert_eq!(
        author_connection.read_block(block).await.metadata,
        b"sealed with content"
    );

    harness.stop().await;
}
