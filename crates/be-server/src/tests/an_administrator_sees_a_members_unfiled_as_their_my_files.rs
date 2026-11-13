use super::a_block_in_unfiled_stays_live_without_a_listing_until_it_is_deleted::make;
use super::the_access_list_leaves_administrators_to_the_organization_line::join;
use super::*;

async fn access(client: &mut TestClient, block: Uuid) -> Option<be_block_graph::Access> {
    client
        .try_read(block).await
        .ok()
        .map(|summary| summary.access)
}

#[tokio::test]
async fn an_administrator_sees_a_members_unfiled_as_their_my_files() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.organization("Company").await;
    let mut boss = harness.member("boss@example.com").await;
    join(
        &mut owner,
        &mut boss,
        organization,
        1,
        "boss@example.com",
        OrganizationRole::Administrator,
    )
    .await;
    let mut member = harness.client().await;
    let member_id = member.register("member@example.com").await;
    join(
        &mut owner,
        &mut member,
        organization,
        2,
        "member@example.com",
        OrganizationRole::Editor,
    )
    .await;
    let mut colleague = harness.member("colleague@example.com").await;
    join(
        &mut owner,
        &mut colleague,
        organization,
        3,
        "colleague@example.com",
        OrganizationRole::Editor,
    )
    .await;

    let my_files = be_block::my_files(organization, member_id);
    let unfiled = be_block::unfiled(organization, member_id);
    let made = make(
        &mut member,
        my_files,
        be_block::WORKSPACE_TYPE,
        BlockParent::Root,
    )
    .await;
    assert!(made.is_ok(), "{made:?}");
    let made = make(
        &mut member,
        unfiled,
        be_block::UNFILED_TYPE,
        BlockParent::Root,
    )
    .await;
    assert!(made.is_ok(), "{made:?}");
    let filed = member.create_block(BlockParent::Block(my_files)).await;
    let unplaced = member.create_block(BlockParent::Block(unfiled)).await;

    for viewer in [&mut boss, &mut colleague] {
        assert_eq!(
            access(viewer, unfiled).await,
            access(viewer, my_files).await,
            "Unfiled is seen as My files is"
        );
        assert_eq!(
            access(viewer, unplaced).await,
            access(viewer, filed).await,
            "and so is what it holds"
        );
    }
}
