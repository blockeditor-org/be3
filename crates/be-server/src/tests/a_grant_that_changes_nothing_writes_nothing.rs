use super::a_graph_change_reaches_only_the_members_who_may_see_it::join;
use super::*;
use be_block_graph::Access;

/// Granting an account the access it already has leaves the block as it was: its version stays,
/// so no one is told of a change and nothing is reannounced.
#[tokio::test]
async fn a_grant_that_changes_nothing_writes_nothing() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("grants").await;
    let block = owner.create_block(BlockParent::Root).await;
    let (_viewer, account) =
        join(&harness, &mut owner, organization, "viewer@example.com", 1).await;

    owner.grant(block, account, Access::View).await.unwrap();
    let granted = owner.read_block(block).await.version;
    owner.grant(block, account, Access::View).await.unwrap();
    assert_eq!(
        owner.read_block(block).await.version,
        granted,
        "granting what was granted changed the block"
    );
    owner.grant(block, account, Access::Edit).await.unwrap();
    assert!(owner.read_block(block).await.version > granted);

    harness.stop().await;
}
