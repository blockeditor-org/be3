use super::*;

use be_type_text::{TextBlock, TextContent};

#[tokio::test]
async fn an_offline_edit_merges_against_the_head_it_left_after_the_server_pruned_it() {
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
            .save(
                note,
                &TextBlock::of("one\ntwo, from the laptop"),
                Some(base)
            )
            .await
            .is_err()
    );
    let ours = offline.local_head(note).unwrap();
    // The phone saves twice within the minute, so the server keeps only its last save: the
    // commits between the laptop's base and the new head are gone.
    let phone = harness.second(&owner).await;
    let first = phone
        .save(note, &TextBlock::of("zero\none"), Some(base))
        .await
        .unwrap()
        .published()
        .unwrap();
    let theirs = phone
        .save(
            note,
            &TextBlock::of("zero, from the phone\none"),
            Some(first),
        )
        .await
        .unwrap()
        .published()
        .unwrap();

    harness.reconnect(&offline, owner.token()).await;
    assert_eq!(
        offline.reconcile(note, Some(theirs)).await.unwrap(),
        Reconciled::Merge {
            base: Some(base),
            ours,
            theirs
        },
        "the merge lost the base the laptop's edit was made on"
    );

    harness.stop().await;
}
