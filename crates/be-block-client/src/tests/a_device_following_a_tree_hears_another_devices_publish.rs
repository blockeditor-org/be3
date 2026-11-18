use super::*;

use be_block_commit::CommitId;
use be_type_text::{TextBlock, TextContent};

use crate::Event;

/// Waits until a tree `peer` follows advances and holds `block` at `head`, reading it from
/// `since` each time it is told; answers the sequence it read to.
async fn heard(
    peer: &Peer<MemoryStore>,
    events: &mut tokio::sync::broadcast::Receiver<Event>,
    block: Uuid,
    head: CommitId,
    mut since: u64,
) -> u64 {
    loop {
        if let Event::TreeAdvanced { entry, .. } = events.recv().await.unwrap() {
            let (changed, _, sequence) = peer.changed_since(entry, since, false).await.unwrap();
            since = sequence;
            if changed
                .iter()
                .any(|summary| summary.id == block && summary.head == Some(head))
            {
                return since;
            }
        }
    }
}

#[tokio::test]
async fn a_device_following_a_tree_hears_another_devices_publish() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let block = phone
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    let first = phone
        .save(block, &TextBlock::of("first"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let laptop = harness.second(&phone).await;
    let mut on_laptop = laptop.connection().subscribe();
    let summary = laptop.summary(block).await.unwrap();
    let entry = summary.entry.expect("the laptop views the block");
    assert_eq!(
        entry, block,
        "a top-level block is its own tree's entry point"
    );
    let (_, _, since) = laptop
        .changed_since(entry, summary.changed, true)
        .await
        .unwrap();

    let second = phone
        .save(block, &TextBlock::of("second"), Some(first))
        .await
        .unwrap()
        .published()
        .unwrap();
    let since = heard(&laptop, &mut on_laptop, block, second, since).await;

    let third = phone
        .save(block, &TextBlock::of("third"), Some(second))
        .await
        .unwrap()
        .published()
        .unwrap();
    heard(&laptop, &mut on_laptop, block, third, since).await;

    harness.stop().await;
}
