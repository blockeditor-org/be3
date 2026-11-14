use super::*;

use be_block_commit::CommitId;
use be_type_text::{TextBlock, TextContent};

use crate::Event;

/// Waits until `changes` tells that `block`'s head is `head`; answers whether it was this
/// connection's own publish.
async fn heard(
    events: &mut tokio::sync::broadcast::Receiver<Event>,
    block: Uuid,
    head: CommitId,
) -> bool {
    loop {
        if let Event::BlockChanged {
            block: changed,
            own,
        } = events.recv().await.unwrap()
            && changed.id == block
            && changed.head == Some(head)
        {
            return own;
        }
    }
}

#[tokio::test]
async fn a_client_tells_its_own_publishes_from_another_devices() {
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
    let mut on_phone = phone.connection().subscribe();
    let mut on_laptop = laptop.connection().subscribe();
    // Reading the block lists it, so `changes` tells each its heads.
    phone.summary(block).await.unwrap();
    laptop.summary(block).await.unwrap();

    let second = phone
        .save(block, &TextBlock::of("second"), Some(first))
        .await
        .unwrap()
        .published()
        .unwrap();
    assert!(
        heard(&mut on_phone, block, second).await,
        "the phone's own publish is its own"
    );
    assert!(
        !heard(&mut on_laptop, block, second).await,
        "the laptop hears the phone's publish as another's"
    );

    let third = laptop
        .save(block, &TextBlock::of("third"), Some(second))
        .await
        .unwrap()
        .published()
        .unwrap();
    assert!(!heard(&mut on_phone, block, third).await);
    assert!(heard(&mut on_laptop, block, third).await);

    harness.stop().await;
}
