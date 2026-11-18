use super::*;

#[tokio::test]
async fn references_keep_the_order_their_block_lists_them_in() {
    let harness = Harness::start().await;
    let mut client = harness.member("presenter@example.com").await;
    let account = client.id();
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
    let mut listed: Vec<Uuid> = Vec::new();
    let mut held_before: Vec<Hash> = Vec::new();
    for (time, references) in [(1_000, &first), (2_000, &moved)] {
        let (commit, chunks) = author.compose(format!("{time}").as_bytes(), time, head);
        client
            .upload(&author.store, &author.objects_of(commit, &chunks))
            .await;
        let response = client
            .call(|peer| {
                peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                    block: deck,
                    expected: head,
                    commit: Some(be_block_protocol::NewCommit {
                        id: commit,
                        parents: head.into_iter().collect(),
                        held: objects_change(&held_before, chunks.clone()),
                        time: time,
                        pinned: false,
                        objects: Vec::new(),
                    }),
                    references: listing(&listed, references.clone()),
                    metadata: Vec::new(),
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
            harness.kept_references(organization, deck).await,
            *references,
            "the server did not keep the order the block listed its references in"
        );
        head = Some(commit);
        listed.clone_from(references);
        held_before = chunks;
    }

    let token = client.token.clone();
    drop(client);
    let harness = harness.restart().await;
    let mut client = harness.second_connection(&token, organization).await;
    assert!(client.read_block(deck).await.head.is_some());
    assert_eq!(
        harness.kept_references(organization, deck).await,
        moved,
        "the order of references was lost when the server reopened"
    );

    harness.stop().await;
}
