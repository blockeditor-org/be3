use be_block::{OrganizationPolicies, WhoMay};
use be_block_graph::Access;
use be_block_protocol::ErrorCode;

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

/// The settings' share with the server can go missing, its grant and key both (the device that
/// made them stopped before sharing them), which no key work names: the next administrator's
/// device to connect shares them again, so the server does not refuse what the policies govern
/// for good.
#[tokio::test]
async fn an_administrators_device_shares_the_settings_with_the_server_again_when_it_connects() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let server = admin.server().expect("the server named its account");
    admin
        .grant(admin.organization_settings(), server.account, Access::None)
        .await
        .unwrap();
    harness
        .database()
        .execute(
            "DELETE FROM wrapped_keys WHERE wrapped_by = ?1",
            [format!("account:{}", server.account)],
        )
        .unwrap();
    let block = admin
        .create_with_id::<be_block::OrganizationPoliciesContent>(
            Uuid::now_v7(),
            BlockParent::Block(admin.organization_settings()),
        )
        .await
        .unwrap();
    let refused = admin
        .save(
            block,
            &be_block::OrganizationPoliciesContent::default(),
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            refused,
            ClientError::Refused(ErrorCode::PolicyUnreadable, _)
        ),
        "the server read settings it was not shared: {refused}"
    );

    let _second = harness.second(&admin).await;
    set_policies(
        &admin,
        &[OrganizationPolicies::set_workspace_creation(Some(
            WhoMay::Administrators,
        ))],
    )
    .await;

    harness.stop().await;
}
