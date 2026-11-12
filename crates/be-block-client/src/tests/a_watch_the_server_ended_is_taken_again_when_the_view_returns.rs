use super::*;
use be_type_workspace::FolderContent;

use be_block_graph::Access;
use be_type_text::{TextBlock, TextContent};

use crate::Event;

/// Waits for the next event `pick` takes.
async fn next<T>(
    events: &mut tokio::sync::broadcast::Receiver<Event>,
    pick: impl Fn(&Event) -> Option<T>,
) -> T {
    loop {
        let event = events.recv().await.expect("the connection stays open");
        if let Some(found) = pick(&event) {
            return found;
        }
    }
}

#[tokio::test]
async fn a_watch_the_server_ended_is_taken_again_when_the_view_returns() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let mine = workspace(&owner, "Mine").await;
    let shared = owner
        .create::<FolderContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    let hidden = owner
        .create::<FolderContent>(BlockParent::Block(mine))
        .await
        .unwrap();
    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .share(shared, guest.account(), Access::View, &public)
        .await
        .unwrap();
    let note = owner
        .create::<TextContent>(BlockParent::Block(shared))
        .await
        .unwrap();
    owner
        .save(note, &TextBlock::of("first"), None)
        .await
        .unwrap();

    let mut events = guest.connection().subscribe();
    guest.children(BlockParent::Block(shared)).await.unwrap();
    guest.watch(note);

    let head = owner.summary(note).await.unwrap().head;
    owner
        .save(note, &TextBlock::of("second"), head)
        .await
        .unwrap();
    next(&mut events, |event| {
        matches!(event, Event::HeadChanged { block, .. } if *block == note).then_some(())
    })
    .await;

    owner
        .set_parent(note, BlockParent::Block(hidden))
        .await
        .unwrap();
    next(&mut events, |event| {
        matches!(event, Event::WatchEnded { block } if *block == note).then_some(())
    })
    .await;

    owner
        .set_parent(note, BlockParent::Block(shared))
        .await
        .unwrap();
    guest.watch(note);
    let head = owner.summary(note).await.unwrap().head;
    owner
        .save(note, &TextBlock::of("third"), head)
        .await
        .unwrap();
    next(&mut events, |event| {
        matches!(event, Event::HeadChanged { block, .. } if *block == note).then_some(())
    })
    .await;

    harness.stop().await;
}
