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
    assert_eq!(
        author.load_commit(marked).await.unwrap().kind,
        be_block_commit::CommitKind::Bookmark("before the cut".to_owned())
    );
    head = Some(marked);
    for step in 20..24 {
        head = author
            .save(block, &image("draft.png", 3_000, step), head)
            .await
            .unwrap()
            .published();
    }

    // Seventeen saves within a minute or two: each publish replaces the one before it in its
    // minute, so only the head, the bookmark and a state per minute crossed are left.
    let after = author.history(block).await.unwrap();
    assert!(
        after.len() <= 4,
        "publishing kept {} of 17 states",
        after.len()
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
