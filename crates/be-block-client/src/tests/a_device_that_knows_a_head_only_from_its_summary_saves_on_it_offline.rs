use be_type_text::{TextBlock, TextContent};

use super::*;

/// A device that learned a block's head from a listing, without its commit, saves an edit on it
/// offline: the head's metadata comes from the summary, which names it, rather than from the
/// server.
#[tokio::test]
async fn a_device_that_knows_a_head_only_from_its_summary_saves_on_it_offline() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let head = owner
        .save(note, &TextBlock::of("written online"), None)
        .await
        .unwrap()
        .published()
        .unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let listed = laptop.children(BlockParent::Block(team)).await.unwrap();
    assert!(
        listed
            .iter()
            .any(|summary| summary.id == note && summary.head == Some(head))
    );
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    // As a device's replica seals: the publish waits in the outbox for the server.
    let edit = TextBlock::of("written offline");
    let sealed = offline
        .seal(
            note,
            Write::encoded(TextContent::CONTENT_TYPE, edit.parts(), Some(head)),
        )
        .await
        .expect("an edit on a head known from its summary is sealed offline");
    assert!(matches!(sealed, Sealed::Sealed(_)), "{sealed:?}");
    assert_eq!(offline.pending().operations, 1, "it waits in the outbox");

    harness.stop().await;
}
