use super::*;

#[tokio::test]
async fn shared_chunks_survive_until_the_last_commit_releases_them() {
    let harness = Harness::start().await;
    let mut client = harness.member("keeper@example.com").await;
    let account = client.id();
    client.organization("notes").await;
    let author = Author::new(account);
    let block = client.create_block(BlockParent::Root).await;

    let body: String = (0..400).map(|line| format!("line {line}\n")).collect();
    let (first, first_chunks) = author.compose(body.as_bytes(), 1_000, None);
    let appended = format!("{body}one more line\n");
    let (second, second_chunks) = author.compose(appended.as_bytes(), 2_000, Some(first));

    let shared: Vec<_> = first_chunks
        .iter()
        .copied()
        .filter(|chunk| second_chunks.contains(chunk))
        .collect();
    assert!(!shared.is_empty(), "the two drafts shared no chunks");

    client
        .upload(&author.store, &author.objects_of(first, &first_chunks))
        .await;
    let _ = client
        .call(|peer| {
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                block: block,
                expected: None,
                commit: Some(be_block_protocol::NewCommit {
                    id: first,
                    parents: Vec::new(),
                    held: objects_change(&[], first_chunks.clone()),
                    time: 1_000,
                    pinned: false,
                    objects: Vec::new(),
                    log_through: None,
                }),
                references: listing(&[], Vec::new()),
                metadata: Vec::new(),
                journal: None,
            })
        })
        .await;
    client
        .upload(&author.store, &author.objects_of(second, &second_chunks))
        .await;
    let _ = client
        .call(|peer| {
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                block: block,
                expected: Some(first),
                commit: Some(be_block_protocol::NewCommit {
                    id: second,
                    parents: Some(first).into_iter().collect(),
                    held: objects_change(&first_chunks, second_chunks.clone()),
                    time: 2_000,
                    pinned: false,
                    objects: Vec::new(),
                    log_through: None,
                }),
                references: listing(&[], Vec::new()),
                metadata: Vec::new(),
                journal: None,
            })
        })
        .await;

    // The second draft replaces the first in its slot, so publishing it pruned the first.
    for chunk in &shared {
        let response = client.download(*chunk).await;
        assert!(
            matches!(response, Ok(Some(_))),
            "a chunk the surviving commit still holds was deleted"
        );
    }

    let response = client.download(first.hash()).await;
    assert!(
        matches!(response, Ok(None)),
        "the pruned commit object survived"
    );

    assert_eq!(client.history(block).await.len(), 1);
    assert_eq!(client.read_block(block).await.head, Some(second));

    harness.stop().await;
}
