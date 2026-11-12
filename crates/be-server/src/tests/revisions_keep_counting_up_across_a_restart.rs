use super::*;

/// A client keeps the revision a listing answered at, and takes a summary newer than it for a
/// change the listing missed, so a server that restarts never hands out a revision again.
#[tokio::test]
async fn revisions_keep_counting_up_across_a_restart() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    client.register("restarter@example.com").await;
    let organization = client.organization("revisions").await;
    for _ in 0..3 {
        client.create_block(BlockParent::Root).await;
    }
    let listed = client
        .call(|peer| client::blocks::list_blocks(peer, Listing::Children(BlockParent::Root)))
        .await
        .expect("listing failed");
    let token = client.token.clone();

    let harness = harness.restart().await;
    let mut client = harness.second_connection(&token, organization).await;
    let made = client.create_block(BlockParent::Root).await;
    assert!(
        client.read_block(made).await.revision > listed.revision,
        "a block made after the restart changes after the listing before it"
    );
}
