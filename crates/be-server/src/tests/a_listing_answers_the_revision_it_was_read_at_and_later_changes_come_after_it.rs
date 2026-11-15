use super::*;

#[tokio::test]
async fn a_listing_answers_the_revision_it_was_read_at_and_later_changes_come_after_it() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    client.register("lister@example.com").await;
    client.organization("listings").await;
    let first = client.create_block(BlockParent::Root).await;

    let listed = client
        .call(|peer| client::blocks::list_blocks(peer, Listing::Children(BlockParent::Root), None))
        .await
        .expect("listing failed");
    let summary = listed
        .blocks
        .iter()
        .find(|summary| summary.id == first)
        .expect("the block is listed");
    assert!(
        summary.revision <= listed.revision,
        "a listed block changed at or before the listing: {} > {}",
        summary.revision,
        listed.revision
    );

    let second = client.create_block(BlockParent::Root).await;
    assert!(
        client.read_block(second).await.revision > listed.revision,
        "a block made after the listing changes after it"
    );
}
