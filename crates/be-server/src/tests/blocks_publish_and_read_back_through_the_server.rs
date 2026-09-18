use super::*;

#[tokio::test]
async fn blocks_publish_and_read_back_through_the_server() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("writer@example.com").await;
    client.workspace("notes").await;
    let author = Author::new(account);

    let block = client.create_block(BlockParent::Root).await;
    assert_eq!(client.read_block(block).await.head, None);

    let (first, chunks) = author.compose(b"first draft\n", 1_000, None);
    client
        .upload(&author.store, &author.objects_of(first, &chunks))
        .await;
    let response = client
        .send(|request| ClientMessage::Publish {
            request,
            block,
            commit: first,
            expected: None,
            chunks: chunks.clone(),
            time: 1_000,
            pinned: false,
            references_added: Vec::new(),
            references_removed: Vec::new(),
        })
        .await;
    assert!(
        matches!(response, ServerMessage::Published { head, .. } if head == first),
        "{response:?}"
    );
    assert_eq!(client.read_block(block).await.head, Some(first));

    let response = client
        .send(|request| ClientMessage::GetObject {
            request,
            hash: chunks[0],
        })
        .await;
    let ServerMessage::Object { bytes: Some(_), .. } = response else {
        panic!("the published chunk was not readable: {response:?}");
    };

    let (second, next_chunks) = author.compose(b"second draft\n", 2_000, Some(first));
    client
        .upload(&author.store, &author.objects_of(second, &next_chunks))
        .await;
    let response = client
        .send(|request| ClientMessage::Publish {
            request,
            block,
            commit: second,
            expected: Some(first),
            chunks: next_chunks,
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

    let history = client.history(block).await;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].commit, second);
    assert_eq!(history[1].commit, first);

    harness.stop().await;
}
