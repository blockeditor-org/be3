use be_block::{OrganizationPolicies, WhoMay};
use be_protocol::ErrorCode;

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

#[tokio::test]
async fn what_a_policy_governs_waits_while_the_server_cannot_read_it() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let server = admin.server().expect("the server named its account");
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
        "the server let through what a policy it could not read governs: {refused}"
    );

    admin.tend().await.unwrap();
    set_policies(
        &admin,
        &[OrganizationPolicies::set_workspace_creation(Some(
            WhoMay::Administrators,
        ))],
    )
    .await;
    let member = harness.member(&admin, "member@example.com").await;
    workspace(&member, "Mine").await;
    let second = member
        .create_block(
            Uuid::now_v7(),
            workspace_ui_types::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::named("Another"),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(second, ClientError::Refused(ErrorCode::PolicyRefused, _)),
        "{second}"
    );

    harness.stop().await;
}
