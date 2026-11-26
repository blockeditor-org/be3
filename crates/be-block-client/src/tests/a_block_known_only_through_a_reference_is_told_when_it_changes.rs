use super::*;

use be_block_content::block_url::block_url;
use be_block_graph::Access;
use be_type_text::{TextBlock, TextContent};

use crate::Event;

/// Waits until `events` tells that `block`, followed as a block known of, changed.
async fn told(events: &mut tokio::sync::broadcast::Receiver<Event>, block: Uuid) {
    loop {
        if let Event::KnownChanged(changed) = events.recv().await.unwrap()
            && changed == block
        {
            return;
        }
    }
}

/// A block a guest knows of only through a reference names no entry point, so no tree it
/// follows tells of it: followed as a block it knows of, it is told as the block changes, and
/// reads its new summary without reconnecting, until it may view it.
#[tokio::test]
async fn a_block_known_only_through_a_reference_is_told_when_it_changes() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let mine = workspace(&owner, "Mine").await;
    let linked = owner
        .create::<TextContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    let first = owner
        .save(linked, &TextBlock::of("a private plan"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let shared = owner
        .create::<TextContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    let link = block_url("https://example.com/", owner.organization(), linked);
    owner
        .save(shared, &TextBlock::of(&format!("Notes, see {link}")), None)
        .await
        .unwrap();
    let public = owner.verified_identity(guest.account()).unwrap();
    owner
        .share(shared, guest.account(), Access::View, &public)
        .await
        .unwrap();

    let mut events = guest.connection().subscribe();
    let known = guest
        .follow_known(linked)
        .await
        .unwrap()
        .expect("the guest knows the linked block exists");
    assert_eq!(known.access, Access::KnowExists);
    assert_eq!(known.entry, None, "it names no entry point to follow");
    assert_eq!(
        guest.follow_known(Uuid::new_v4()).await.unwrap(),
        None,
        "a block the guest may not know of reads as none"
    );

    owner
        .save(linked, &TextBlock::of("a plan, revised"), Some(first))
        .await
        .unwrap();
    told(&mut events, linked).await;
    let changed = guest.read_summaries(&[linked]).await.unwrap();
    assert!(
        changed[0].changed > known.changed,
        "the guest reads the block's new summary"
    );
    assert_eq!(changed[0].access, Access::KnowExists);

    owner
        .share(linked, guest.account(), Access::View, &public)
        .await
        .unwrap();
    told(&mut events, linked).await;
    let opened = guest.read_summaries(&[linked]).await.unwrap();
    assert_eq!(
        opened[0].access,
        Access::View,
        "a grant reaches the guest without a reconnect"
    );

    harness.stop().await;
}
