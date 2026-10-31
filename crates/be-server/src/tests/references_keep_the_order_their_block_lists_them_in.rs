use super::*;

#[tokio::test]
async fn references_keep_the_order_their_block_lists_them_in() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("presenter@example.com").await;
    let organization = client.organization("talks").await;
    let author = Author::new(account);

    let deck = client.create_block(BlockParent::Root).await;
    let mut slides: Vec<Uuid> = Vec::new();
    for _ in 0..4 {
        slides.push(client.create_block(BlockParent::Block(deck)).await);
    }
    slides.sort_unstable();
    let first = vec![slides[2], slides[0], slides[3], slides[1]];
    let moved = vec![slides[3], slides[2], slides[1], slides[0]];

    let mut head = None;
    for (time, references) in [(1_000, &first), (2_000, &moved)] {
        let (commit, chunks) = author.compose(format!("{time}").as_bytes(), time, head);
        client
            .upload(&author.store, &author.objects_of(commit, &chunks))
            .await;
        let response = client
            .call(|peer| {
                client::blocks::publish(
                    peer,
                    deck,
                    commit,
                    head,
                    head.into_iter().collect(),
                    chunks,
                    time,
                    false,
                    references.clone(),
                    Vec::new(),
                    Vec::new(),
                )
            })
            .await;
        assert!(
            matches!(response, Ok(be_protocol::PublishOutcome::Published(_))),
            "{response:?}"
        );
        assert_eq!(
            client.read_block(deck).await.references,
            *references,
            "the server did not keep the order the block listed its references in"
        );
        head = Some(commit);
    }

    let token = client.token.clone();
    drop(client);
    let harness = harness.restart().await;
    let mut client = harness.second_connection(&token, organization).await;
    assert_eq!(
        client.read_block(deck).await.references,
        moved,
        "the order of references was lost when the server reopened"
    );

    harness.stop().await;
}
