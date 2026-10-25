use be_block::{OrganizationPolicies, WhoMay};
use be_protocol::OrganizationRole;

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

#[tokio::test]
async fn a_second_administrator_edits_the_organizations_settings() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    admin
        .set_role(deputy.account(), OrganizationRole::Administrator)
        .await
        .unwrap();
    let promoted = harness.second(&deputy).await;
    let settings = promoted.organization_settings();
    let key = promoted.keys().scope_key(settings).unwrap();
    assert!(
        !promoted.keys().trusted(key),
        "the server's copy of the settings key was trusted"
    );

    admin.tend().await.unwrap();
    let promoted = harness.second(&deputy).await;
    assert!(promoted.keys().trusted(key));
    set_policies(
        &promoted,
        &[OrganizationPolicies::set_workspace_creation(Some(
            WhoMay::Administrators,
        ))],
    )
    .await;
    assert_eq!(
        admin
            .organization_policies()
            .await
            .unwrap()
            .workspace_creation(),
        WhoMay::Administrators
    );

    harness.stop().await;
}
