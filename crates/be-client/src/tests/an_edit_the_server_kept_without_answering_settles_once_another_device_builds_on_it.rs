use super::*;

use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn an_edit_the_server_kept_without_answering_settles_once_another_device_builds_on_it() {
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
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    assert!(
        offline
            .save(note, &TextBlock::of("two"), Some(base))
            .await
            .is_err()
    );
    let two = offline.local_head(note).unwrap();
    let answered = harness.offline(&owner, &refs, &store);
    harness.reconnect(&answered, owner.token()).await;
    assert_eq!(
        answered.drain(|_| false).await.unwrap(),
        [Drained::Published {
            block: note,
            head: two
        }]
    );
    let phone = harness.second(&owner).await;
    let three = phone
        .save(note, &TextBlock::of("three"), Some(two))
        .await
        .unwrap()
        .published()
        .unwrap();

    harness.reconnect(&offline, owner.token()).await;
    assert_eq!(
        offline.drain(|_| false).await.unwrap(),
        [Drained::Published {
            block: note,
            head: three
        }],
        "an edit the server already holds was taken for a diverged one"
    );
    assert_eq!(offline.pending(), refs::PendingCounts::default());
    assert_eq!(offline.local_head(note), Some(three));
    assert_eq!(text(&offline, note).await, "three");

    harness.stop().await;
}
