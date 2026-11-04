use be_block::{
    OrganizationPolicies, OrganizationPoliciesContent, Root, Settings, SettingsContent, WhoMay,
};

use super::*;

#[tokio::test]
async fn a_policy_outside_the_organizations_settings_is_not_enforced() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let team = workspace(&admin, "Team").await;
    let block = admin
        .create::<OrganizationPoliciesContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let mut policies = OrganizationPoliciesContent::default();
    policies.apply(&OrganizationPolicies::set_workspace_creation(Some(
        WhoMay::Administrators,
    )));
    admin.save(block, &policies, None).await.unwrap();
    admin.share_with_server(block).await.unwrap();
    let settings = admin.organization_settings();
    let mut content = SettingsContent::default();
    content.apply(&Settings::set_policy(
        OrganizationPolicies::CONTENT_TYPE,
        block,
    ));
    let head = admin.summary(settings).await.unwrap().head;
    admin.save(settings, &content, head).await.unwrap();

    let member = harness.member(&admin, "member@example.com").await;
    workspace(&member, "Mine").await;
    workspace(&member, "Another").await;
    harness.stop().await;
}
