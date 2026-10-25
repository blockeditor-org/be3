use be_block::{OrganizationPolicies, WhoMay};
use be_protocol::ErrorCode;

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

#[tokio::test]
async fn moving_a_block_to_the_top_follows_who_may_make_workspaces() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    set_policies(
        &admin,
        &[OrganizationPolicies::set_workspace_creation(Some(
            WhoMay::Administrators,
        ))],
    )
    .await;
    let member = harness.member(&admin, "member@example.com").await;
    let mine = workspace(&member, "Mine").await;
    let folder = member
        .create::<text_block_types::TextContent>(BlockParent::Block(mine))
        .await
        .unwrap();

    let moved = member.set_parent(folder, BlockParent::Root).await;
    assert!(
        matches!(moved, Err(ClientError::Refused(ErrorCode::PolicyRefused, _))),
        "a member moved a block to the top past the policy: {moved:?}"
    );
    assert_eq!(
        member.summary(folder).await.unwrap().parent,
        Some(BlockParent::Block(mine))
    );

    let team = workspace(&admin, "Team").await;
    let inside = admin
        .create::<text_block_types::TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    admin.set_parent(inside, BlockParent::Root).await.unwrap();
    assert_eq!(
        admin.summary(inside).await.unwrap().parent,
        Some(BlockParent::Root),
        "an administrator still makes one"
    );
    harness.stop().await;
}
