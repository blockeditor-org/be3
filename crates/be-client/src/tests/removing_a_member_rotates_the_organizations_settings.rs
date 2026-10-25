use be_block::{OrganizationPolicies, OrganizationPoliciesContent, WhoMay};
use be_protocol::{ErrorCode, KeyKind};

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

#[tokio::test]
async fn removing_a_member_rotates_the_organizations_settings() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let block = set_policies(&admin, &[OrganizationPolicies::set_history_days(Some(30))]).await;
    let settings = admin.organization_settings();
    let staying = harness.member(&admin, "staying@example.com").await;
    let leaving = harness.member(&admin, "leaving@example.com").await;
    assert_eq!(
        leaving.organization_policies().await.unwrap().history_days,
        Some(30)
    );
    let before = admin.keys().scope_key(settings).unwrap();

    admin.remove_member(leaving.account()).await.unwrap();
    assert!(
        admin
            .key_work()
            .await
            .unwrap()
            .rotations
            .contains(&settings),
        "a removed member still holds the key to what the organization's settings say next"
    );
    admin.tend().await.unwrap();
    assert!(admin.key_work().await.unwrap().rotations.is_empty());
    let after = admin.keys().scope_key(settings).unwrap();
    assert_ne!(after, before);
    assert!(
        admin
            .keys()
            .keys_of(block, KeyKind::Data)
            .last()
            .is_some_and(|row| admin
                .keys()
                .wrapped_by(row.id)
                .contains(&be_protocol::WrappedBy::Key(after)))
    );

    let mut policies = OrganizationPoliciesContent::default();
    policies.apply(&OrganizationPolicies::set_workspace_creation(Some(
        WhoMay::Administrators,
    )));
    let head = admin.summary(block).await.unwrap().head;
    admin.save(block, &policies, head).await.unwrap();
    let reopened = harness.second(&staying).await;
    assert_eq!(
        reopened
            .organization_policies()
            .await
            .unwrap()
            .workspace_creation(),
        WhoMay::Administrators
    );
    workspace(&reopened, "Mine").await;
    let second = reopened
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
        "the server could not read the rotated policies: {second}"
    );

    harness.stop().await;
}
