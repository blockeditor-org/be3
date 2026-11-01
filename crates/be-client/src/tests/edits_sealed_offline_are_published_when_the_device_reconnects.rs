use super::*;

use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn edits_sealed_offline_are_published_when_the_device_reconnects() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let team = workspace(&laptop, "Team").await;
    let note = laptop
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let base = laptop
        .save(note, &TextBlock::of("one"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    laptop.children(BlockParent::Block(team)).await.unwrap();
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    assert!(matches!(
        offline.save(note, &TextBlock::of("two"), Some(base)).await,
        Err(ClientError::Disconnected(_))
    ));
    let two = offline.local_head(note).unwrap();
    assert_ne!(two, base);
    assert_eq!(offline.known_remote_head(note), Some(base));
    assert!(matches!(
        offline.save(note, &TextBlock::of("three"), Some(two)).await,
        Err(ClientError::Disconnected(_))
    ));
    let three = offline.local_head(note).unwrap();
    assert_eq!(offline.pending_commits(note), [two, three]);
    assert_eq!(text(&offline, note).await, "three");

    let added = offline
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    assert!(matches!(
        offline
            .save(added, &TextBlock::of("made offline"), None)
            .await,
        Err(ClientError::Disconnected(_))
    ));
    let listed = offline.children(BlockParent::Block(team)).await.unwrap();
    assert!(listed.iter().any(|block| block.id == added));
    let counts = offline.pending();
    assert_eq!(counts.operations, 4);
    assert_eq!(counts.commits, 3);
    assert_eq!(counts.blocks, 2);
    assert_eq!(counts.unsent, 0);

    let resumed = harness.offline(&owner, &refs, &store);
    assert_eq!(
        resumed.pending(),
        counts,
        "the outbox did not survive a restart"
    );
    harness.reconnect(&resumed, owner.token()).await;
    let drained = resumed.drain(|_| false).await.unwrap();
    assert!(drained.contains(&Drained::Published {
        block: note,
        head: three
    }));
    assert!(drained.contains(&Drained::Created { block: added }));
    assert!(
        drained
            .iter()
            .any(|outcome| matches!(outcome, Drained::Published { block, .. } if *block == added))
    );
    assert_eq!(resumed.pending(), refs::PendingCounts::default());
    assert_eq!(resumed.known_remote_head(note), Some(three));

    let phone = harness.second(&owner).await;
    assert_eq!(text(&phone, note).await, "three");
    assert_eq!(text(&phone, added).await, "made offline");
    let history: Vec<_> = phone
        .history(note)
        .await
        .unwrap()
        .into_iter()
        .map(|entry| entry.commit)
        .collect();
    assert!(history.contains(&two) && history.contains(&base));

    let theirs = phone
        .save(note, &TextBlock::of("four, from the phone"), Some(three))
        .await
        .unwrap()
        .published()
        .unwrap();
    let lagging = harness.offline(&owner, &refs, &store);
    assert!(
        lagging
            .save(note, &TextBlock::of("four, from the laptop"), Some(three))
            .await
            .is_err()
    );
    let ours = lagging.local_head(note).unwrap();
    harness.reconnect(&lagging, owner.token()).await;
    assert_eq!(
        lagging.drain(|_| false).await.unwrap(),
        [Drained::Diverged {
            block: note,
            local: ours,
            remote: Some(theirs)
        }]
    );
    assert_eq!(lagging.pending().commits, 1, "a diverged edit was dropped");
    assert_eq!(lagging.known_remote_head(note), Some(theirs));
    let merged = lagging
        .write(
            note,
            Write::encoded(
                TextContent::CONTENT_TYPE,
                &TextBlock::of("four, merged").encode(),
                Some(theirs),
            )
            .merging(Some(three), ours),
        )
        .await
        .unwrap();
    assert!(matches!(merged, Saved::Published(_)));
    assert_eq!(lagging.pending(), refs::PendingCounts::default());
    assert_eq!(text(&phone, note).await, "four, merged");

    harness.stop().await;
}
