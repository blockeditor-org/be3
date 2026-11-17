use super::*;

#[tokio::test]
async fn metadata_travels_with_the_content_once_a_block_has_any() {
    let harness = Harness::start().await;
    let mut author_connection = harness.member("author@example.com").await;
    let account = author_connection.id();
    let organization = author_connection.organization("notes").await;
    let block = author_connection.create_block(BlockParent::Root).await;
    let mut other = harness
        .second_connection(&author_connection.token.clone(), organization)
        .await;
    other.follow(block).await;

    let response = author_connection
        .call(|peer| {
            peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::SetMetadata {
                block: block,
                metadata: b"sealed before content".to_vec(),
                references: Vec::new(),
            })
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
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                block: block,
                commit: head,
                expected: None,
                parents: Vec::new(),
                held: objects_change(&[], chunks),
                time: 1_000,
                pinned: false,
                references: listing(&[], Vec::new()),
                metadata: b"sealed with content".to_vec(),
                objects: Vec::new(),
                journal: None,
            })
        })
        .await;
    assert!(
        matches!(
            response,
            Ok(be_block_protocol::PublishOutcome::Published(_))
        ),
        "{response:?}"
    );
    assert_eq!(
        author_connection.read_block(block).await.metadata,
        b"sealed with content"
    );
    let heard = other
        .heard_change(block, |changed| changed.head == Some(head))
        .await;
    assert_eq!(heard.metadata, b"sealed with content");

    let response = author_connection
        .call(|peer| {
            peer.write::<be_block_protocol::BlockSummary>(be_block_protocol::Op::SetMetadata {
                block: block,
                metadata: b"sealed beside content".to_vec(),
                references: Vec::new(),
            })
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
