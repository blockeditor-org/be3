use be_block::{OrganizationPolicies, WhoMay};
use be_protocol::ErrorCode;

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

#[tokio::test]
async fn what_a_policy_governs_waits_while_the_server_cannot_read_it() {
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
    let server = admin.server().expect("the server named its account");
    let settings = admin.organization_settings();

    admin
        .grant(settings, server.account, be_graph::Access::None)
        .await
        .unwrap();
    let refused = member
        .create_block(
            Uuid::now_v7(),
            workspace_ui_types::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::named("Mine"),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            refused,
            ClientError::Refused(ErrorCode::PolicyUnreadable, _)
        ),
        "the server let through what a policy it could not read governs: {refused}"
    );

    admin.share_with_server(settings).await.unwrap();
    workspace(&member, "Mine").await;

    harness.stop().await;
}
