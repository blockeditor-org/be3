use super::*;

#[tokio::test]
async fn history_is_thinned_but_the_head_and_bookmarks_survive() {
    let harness = Harness::start().await;
    let author = harness.owner("keeper@example.com").await;
    let block = author
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();

    let mut head = None;
    for step in 0..12 {
        let saved = author
            .save(block, &image("draft.png", 3_000, step + 1), head)
            .await
            .unwrap();
        head = saved.published();
        assert!(head.is_some(), "a save in a clean chain was rejected");
    }
    let marked = author
        .bookmark(
            block,
            &image("draft.png", 3_000, 99),
            head,
            "before the cut",
        )
        .await
        .unwrap()
        .published()
        .unwrap();
    head = Some(marked);
    for step in 20..24 {
        head = author
            .save(block, &image("draft.png", 3_000, step), head)
            .await
            .unwrap()
            .published();
    }

    let before = author.history(block).await.unwrap();
    assert_eq!(before.len(), 17);
    assert!(before.iter().any(|entry| entry.pinned));

    let policy = RetentionPolicy {
        keep_all_within: 0,
        buckets: vec![be_commit::retention::Bucket {
            until_age: i64::MAX,
            interval: 10 * MINUTE,
        }],
    };
    author.prune(block, &policy).await.unwrap();

    let after = author.history(block).await.unwrap();
    assert!(
        after.len() < before.len(),
        "pruning kept every one of {} states",
        before.len()
    );
    assert_eq!(after[0].commit, head.unwrap(), "the head was pruned away");
    assert!(
        after.iter().any(|entry| entry.commit == marked),
        "a bookmark was pruned away"
    );
    assert_eq!(
        author.open::<ImageContent>(block).await.unwrap().unwrap(),
        image("draft.png", 3_000, 23)
    );

    harness.stop().await;
}
