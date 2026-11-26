use be_block::{OrganizationPolicies, OrganizationPoliciesContent, WhoMay};
use be_block_protocol::OrganizationRole;

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

/// A policies block's key reaches another administrator wrapped under the settings key, which
/// the server holds and which so vouches for nothing; once the device that made the policies
/// tends, it seals that key to the other administrator as itself, and the other administrator
/// edits the policies in place under a key it trusts.
#[tokio::test]
async fn an_administrator_edits_policies_another_administrator_made() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let policies = set_policies(
        &admin,
        &[OrganizationPolicies::set_workspace_creation(Some(
            WhoMay::Administrators,
        ))],
    )
    .await;
    admin
        .set_role(deputy.account(), OrganizationRole::Administrator)
        .await
        .unwrap();
    admin.tend().await.unwrap();

    let promoted = harness.second(&deputy).await;
    let head = promoted.summary(policies).await.unwrap().head;
    let mut content = promoted
        .open::<OrganizationPoliciesContent>(policies)
        .await
        .unwrap()
        .expect("the policies have content");
    content.apply(&OrganizationPolicies::set_workspace_creation(Some(
        WhoMay::Everyone,
    )));
    promoted
        .save(policies, &content, head)
        .await
        .expect("the other administrator edits the policies in place");
    assert_eq!(
        admin
            .organization_policies()
            .await
            .unwrap()
            .workspace_creation(),
        WhoMay::Everyone
    );
}
