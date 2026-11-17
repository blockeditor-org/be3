use super::*;

use be_block_graph::Access;

/// A block made under one whose log root this device has not pinned asks the server for that
/// block's keys first. Answered with none that opens, the parent is locked
/// (`ClientError::Locked`); not asked, for want of a connection, it is out of reach, which
/// passes with the next one: never a lock.
#[tokio::test]
async fn a_block_made_under_one_whose_keys_cannot_be_asked_for_is_not_locked() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let shared = workspace(&owner, "Shared").await;
    owner
        .grant(shared, guest.account(), Access::Edit)
        .await
        .unwrap();
    let listed = vec![guest.summary(shared).await.unwrap()];
    guest.learn(&listed).await.unwrap();

    // The keys answered, and none of them is the workspace's.
    let made = guest.create::<TextContent>(BlockParent::Block(shared)).await;
    assert!(
        matches!(made, Err(ClientError::Locked(locked)) if locked == shared),
        "{made:?}"
    );

    // Offline, nothing answered.
    guest.detach();
    let made = guest.create::<TextContent>(BlockParent::Block(shared)).await;
    match made {
        Err(error) => assert!(error.is_unreachable(), "{error:?}"),
        Ok(block) => panic!("{block} was made under a parent whose root is not pinned"),
    }

    harness.stop().await;
}
