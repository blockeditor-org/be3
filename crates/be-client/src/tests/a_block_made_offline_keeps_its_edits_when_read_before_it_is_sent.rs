use super::*;

use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_block_made_offline_keeps_its_edits_when_read_before_it_is_sent() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let team = workspace(&laptop, "Team").await;
    laptop.children(BlockParent::Block(team)).await.unwrap();
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    let added = offline
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    assert!(
        offline
            .save(added, &TextBlock::of("made offline"), None)
            .await
            .is_err()
    );
    harness.reconnect(&offline, owner.token()).await;
    assert!(matches!(
        offline.summary(added).await,
        Err(ClientError::Refused(..))
    ));
    let counts = offline.pending();
    assert_eq!(
        (counts.operations, counts.unsent),
        (2, 0),
        "a block not sent yet was taken for one this account lost"
    );
    let drained = offline.drain(|_| false).await.unwrap();
    assert!(drained.contains(&Drained::Created { block: added }));
    assert_eq!(offline.pending(), refs::PendingCounts::default());
    let phone = harness.second(&owner).await;
    assert_eq!(text(&phone, added).await, "made offline");

    harness.stop().await;
}
