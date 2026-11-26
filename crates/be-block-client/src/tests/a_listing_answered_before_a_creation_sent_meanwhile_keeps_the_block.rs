use be_type_text::TextContent;
use futures_util::FutureExt;

use super::*;

/// A listing asked for while a block's creation waits in the outbox, whose answer is taken only
/// after the drain sent that creation, is from before the server made the block (calls on one
/// connection are answered in order): it is laid over with what the outbox held when it was
/// asked, so the block made here stays listed.
#[tokio::test]
async fn a_listing_answered_before_a_creation_sent_meanwhile_keeps_the_block() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let team = workspace(&owner, "Team").await;
    let folder = Listing::Children(BlockParent::Block(team));
    owner.list(folder).await.unwrap();
    let block = Uuid::now_v7();
    owner
        .keep_creation(
            block,
            TextContent::CONTENT_TYPE,
            BlockParent::Block(team),
            &be_block_content::BlockMetadata::named("Plan"),
            false,
        )
        .await
        .expect("the block is kept here");

    // Asked for before the drain sends the creation, so answered from before it.
    let mut listing = Box::pin(owner.listing(folder));
    assert!(
        listing.as_mut().now_or_never().is_none(),
        "the listing waits for the server's answer"
    );
    let drained = owner.drain(|_| false).await.unwrap();
    assert!(
        drained
            .iter()
            .any(|outcome| matches!(outcome, Drained::Created { block: made } if *made == block)),
        "the drain sent the creation: {drained:?}"
    );
    assert!(
        owner
            .list(folder)
            .await
            .unwrap()
            .iter()
            .any(|summary| summary.id == block),
        "the server lists the block once it made it"
    );

    let listed = listing.await.unwrap();
    assert!(
        listed.blocks.iter().any(|summary| summary.id == block),
        "an answer from before the creation reached the server leaves out the block made here, \
         sent while it was asked: {:?}",
        listed.blocks
    );

    harness.stop().await;
}
