use super::*;

#[tokio::test]
async fn a_reply_before_a_change_and_the_change_after_it_keep_their_order_across_a_restart() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("author@example.com").await;
    let organization = client.organization("notes").await;
    let folder = client.create_block(BlockParent::Root).await;
    let inside = client.create_block(BlockParent::Block(folder)).await;
    let listed = client
        .call(|peer| {
            peer.read::<be_block_protocol::Listed>(be_block_protocol::Query::List {
                listing: Listing::Children(BlockParent::Block(folder)),
                after: None,
            })
        })
        .await
        .expect("listing failed");
    let before = client.read_block(inside).await;
    assert!(before.changed <= listed.sequence);
    let token = client.token.clone();
    client.close().await;

    let harness = harness.restart().await;
    let mut writer = harness.second_connection(&token, organization).await;
    let mut reader = harness.second_connection(&token, organization).await;
    reader.follow(folder).await;
    let author = Author::new(account);
    let (head, chunks) = author.compose(b"after the restart\n", 1_000, None);
    let published = writer
        .publish(&author, inside, head, chunks, None, 1_000)
        .await;
    assert!(published.is_ok(), "{published:?}");
    let after = reader
        .heard_change(inside, |summary| summary.head == Some(head))
        .await;
    assert!(
        after.changed > before.changed && after.changed > listed.sequence,
        "a change after a restart is ordered after what was read before it: {} after {} and {}",
        after.changed,
        before.changed,
        listed.sequence
    );

    harness.stop().await;
}
