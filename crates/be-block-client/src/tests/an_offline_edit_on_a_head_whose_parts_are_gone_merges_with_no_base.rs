use super::*;

use be_block_store::{FileStore, ObjectStore};
use be_type_text::{TextBlock, TextContent};

/// The laptop holds the commit its offline edit was made on but not its content any more, and
/// the phone's saves pruned it on the server: a base whose commit is held but whose parts load
/// nowhere is no base either, so the merge has none rather than one it can never read.
#[tokio::test]
async fn an_offline_edit_on_a_head_whose_parts_are_gone_merges_with_no_base() {
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
    let manifest = laptop.load_reached(base).await.unwrap().manifest;
    let parts = laptop.objects_of(&manifest).await.unwrap();
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
            .is_ok_and(|saved| matches!(saved, Saved::Kept(_)))
    );
    let ours = offline.local_head(note).unwrap();
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
    // The base's parts are gone everywhere; its commit stays on the laptop.
    let objects = FileStore::open(harness.server.directory.join("objects")).unwrap();
    for part in parts {
        store.remove(part).unwrap();
        objects.remove(part).unwrap();
    }

    harness.reconnect(&offline, owner.token()).await;
    assert_eq!(
        offline.reconcile(note, Some(theirs)).await.unwrap(),
        Reconciled::Merge {
            base: None,
            ours,
            theirs
        },
        "the merge took a base whose parts load nowhere"
    );

    harness.stop().await;
}
