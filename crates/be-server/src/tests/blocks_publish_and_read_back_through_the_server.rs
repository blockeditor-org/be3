use super::*;

#[tokio::test]
async fn blocks_publish_and_read_back_through_the_server() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("writer@example.com").await;
    client.organization("notes").await;
    let author = Author::new(account);

    let block = client.create_block(BlockParent::Root).await;
    assert_eq!(client.read_block(block).await.head, None);

    let (first, chunks) = author.compose(b"first draft\n", 1_000, None);
    client
        .upload(&author.store, &author.objects_of(first, &chunks))
        .await;
    let response = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                first,
                None,
                Vec::new(),
                chunks.clone(),
                1_000,
                false,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(
        matches!(response, Ok(be_protocol::PublishOutcome::Published(head)) if head == first),
        "{response:?}"
    );
    assert_eq!(client.read_block(block).await.head, Some(first));

    let response = client
        .call(|peer| client::objects::get_object(peer, chunks[0]))
        .await;
    let Ok(Some(_)) = response else {
        panic!("the published chunk was not readable: {response:?}");
    };

    let (second, next_chunks) = author.compose(b"second draft\n", 2_000, Some(first));
    let carried = author
        .objects_of(second, &next_chunks)
        .into_iter()
        .map(|hash| author.store.get(hash).unwrap().unwrap())
        .collect();
    let response = client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                second,
                Some(first),
                Some(first).into_iter().collect(),
                next_chunks,
                2_000,
                false,
                Vec::new(),
                Vec::new(),
                carried,
            )
        })
        .await;
    assert!(
        matches!(response, Ok(be_protocol::PublishOutcome::Published(_))),
        "{response:?}"
    );

    let history = client.history(block).await;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].commit, second);
    assert_eq!(history[1].commit, first);

    harness.stop().await;
}
