use super::*;

#[tokio::test]
async fn blocks_keep_the_order_they_were_created_in_across_a_restart() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let workspace = owner.workspace("order").await;
    let first = owner.create_block(BlockParent::Root).await;
    let second = owner.create_block(BlockParent::Root).await;
    let third = owner.create_block(BlockParent::Block(first)).await;
    let created = [
        owner.read_block(first).await.created,
        owner.read_block(second).await.created,
        owner.read_block(third).await.created,
    ];
    assert!(
        created[0] < created[1] && created[1] < created[2],
        "{created:?}"
    );

    let token = owner.token.clone();
    drop(owner);
    let harness = harness.restart().await;
    let mut owner = harness.second_connection(&token, workspace).await;
    assert_eq!(owner.read_block(first).await.created, created[0]);
    assert_eq!(owner.read_block(third).await.created, created[2]);
    let fourth = owner.create_block(BlockParent::Root).await;
    assert!(owner.read_block(fourth).await.created > created[2]);

    harness.stop().await;
}
