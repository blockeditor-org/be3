use super::*;

#[tokio::test]
async fn a_burst_of_publishes_reaches_a_follower_as_a_few_tree_advances() {
    let harness = Harness::start().await;
    let mut writer = harness.client().await;
    let account = writer.register("author@example.com").await;
    let organization = writer.organization("notes").await;
    let block = writer.create_block(BlockParent::Root).await;
    let mut follower = harness
        .second_connection(&writer.token.clone(), organization)
        .await;
    follower.follow(block).await;

    // A hundred publishes, sent together, while the follower reads nothing.
    let author = Author::new(account);
    let mut expected = None;
    let mut sent = Vec::new();
    for index in 0..100 {
        let (head, chunks) = author.compose(
            format!("draft {index}\n").as_bytes(),
            1_000 + index,
            expected,
        );
        writer
            .upload(&author.store, &author.objects_of(head, &chunks))
            .await;
        let before = author.held_by(expected);
        sent.push(writer.peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish { block: block, commit: head, expected: expected, parents: expected.into_iter().collect(), held: objects_change(&before, chunks), time: 1_000 + index, pinned: false, references: listing(&[], Vec::new()), metadata: Vec::new(), objects: Vec::new(), journal: None }));
        expected = Some(head);
    }
    for published in sent {
        let published = published.await.expect("the connection stayed open");
        assert!(published.is_ok(), "{published:?}");
    }
    let last = expected.unwrap();

    // Told once, then once more for whatever came after it read: never once a publish.
    let mut advances = 0;
    loop {
        let Notice::TreeAdvanced { entry, .. } = follower.graph_change().await else {
            continue;
        };
        advances += 1;
        let (summaries, _) = follower.read_changes(entry).await;
        if summaries.iter().any(|summary| summary.head == Some(last)) {
            break;
        }
    }
    follower.round_trip().await;
    let after = follower
        .notifications
        .iter()
        .filter(|notice| matches!(notice, Notice::TreeAdvanced { .. }))
        .count();
    assert!(
        advances + after <= 3,
        "a burst of a hundred publishes was told {} times",
        advances + after
    );

    harness.stop().await;
}
