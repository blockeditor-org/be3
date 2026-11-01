use super::*;

use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_merge_left_waiting_is_sent_once_the_device_reconnects() {
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
            .save(note, &TextBlock::of("two, from the laptop"), Some(base))
            .await
            .is_err()
    );
    let ours = offline.local_head(note).unwrap();
    let phone = harness.second(&owner).await;
    let theirs = phone
        .save(note, &TextBlock::of("two, from the phone"), Some(base))
        .await
        .unwrap()
        .published()
        .unwrap();

    harness.reconnect(&offline, owner.token()).await;
    offline.fetch_history(theirs).await.unwrap();
    let merged = offline
        .seal(
            note,
            Write::encoded(
                TextContent::CONTENT_TYPE,
                &TextBlock::of("two, merged").encode(),
                Some(theirs),
            )
            .merging(Some(base), ours),
        )
        .await
        .unwrap();
    let Sealed::Sealed(merged) = merged else {
        panic!("the merge was not sealed");
    };
    assert_eq!(offline.pending().commits, 2);

    let resumed = harness.offline(&owner, &refs, &store);
    harness.reconnect(&resumed, owner.token()).await;
    assert_eq!(
        resumed.drain(|_| false).await.unwrap(),
        [Drained::Diverged {
            block: note,
            local: merged,
            remote: Some(theirs)
        }]
    );
    assert_eq!(
        resumed.reconcile(note, Some(theirs)).await.unwrap(),
        Reconciled::Done,
        "the merge sealed before the publish was lost was not sent"
    );
    assert_eq!(resumed.pending(), refs::PendingCounts::default());
    assert_eq!(resumed.known_remote_head(note), Some(merged));
    assert_eq!(text(&phone, note).await, "two, merged");

    harness.stop().await;
}
