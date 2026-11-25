use super::*;

use be_type_text::{TextBlock, TextContent};

/// The laptop seals an offline edit on a head it knows only from a listing, never fetched (the
/// phone's), as a replica following its holder does, and the phone's next save prunes that head
/// on the server: the common ancestor both sides name is held nowhere, so the merge has no base
/// rather than one it can never load.
#[tokio::test]
async fn an_offline_edit_on_a_head_it_never_held_merges_with_no_base_once_the_server_pruned_it() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let heard = owner
        .save(note, &TextBlock::of("one"), None)
        .await
        .unwrap()
        .published()
        .unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    laptop.children(BlockParent::Block(team)).await.unwrap();
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    let edit = TextBlock::of("one\ntwo, from the laptop");
    let sealed = offline
        .seal(
            note,
            Write::encoded(TextContent::CONTENT_TYPE, edit.parts(), Some(heard)),
        )
        .await
        .unwrap();
    let Sealed::Sealed(ours) = sealed else {
        panic!("the edit is sealed offline: {sealed:?}");
    };
    let theirs = owner
        .save(note, &TextBlock::of("zero, from the phone\none"), Some(heard))
        .await
        .unwrap()
        .published()
        .unwrap();

    harness.reconnect(&offline, owner.token()).await;
    assert_eq!(
        offline.reconcile(note, Some(theirs)).await.unwrap(),
        Reconciled::Merge {
            base: None,
            ours,
            theirs
        },
        "the merge took a base held nowhere"
    );

    harness.stop().await;
}
