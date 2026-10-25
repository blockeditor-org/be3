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
        .send(|request| ClientMessage::SetMetadata {
            request,
            block,
            metadata: b"sealed before content".to_vec(),
            references: Vec::new(),
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Block { .. }),
        "{response:?}"
    );

    let author = Author::new(account);
    let (head, chunks) = author.compose(b"first\n", 1_000, None);
    author_connection
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    let response = author_connection
        .send(|request| ClientMessage::Publish {
            request,
            block,
            commit: head,
            expected: None,
            parents: Vec::new(),
            chunks,
            time: 1_000,
            pinned: false,
            references: Vec::new(),
            metadata: b"sealed with content".to_vec(),
            objects: Vec::new(),
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Published { .. }),
        "{response:?}"
    );
    assert_eq!(
        author_connection.read_block(block).await.metadata,
        b"sealed with content"
    );
    let heard = loop {
        if let ServerMessage::BlockChanged { block: changed } = other.graph_change().await
            && changed.id == block
            && changed.head == Some(head)
        {
            break changed;
        }
    };
    assert_eq!(heard.metadata, b"sealed with content");

    let response = author_connection
        .send(|request| ClientMessage::SetMetadata {
            request,
            block,
            metadata: b"sealed beside content".to_vec(),
            references: Vec::new(),
        })
        .await;
    assert!(
        matches!(
            response,
            ServerMessage::Failed {
                code: ErrorCode::BlockHasContent,
                ..
            }
        ),
        "{response:?}"
    );
    assert_eq!(
        author_connection.read_block(block).await.metadata,
        b"sealed with content"
    );

    harness.stop().await;
}
