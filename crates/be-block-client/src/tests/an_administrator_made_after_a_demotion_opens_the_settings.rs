use super::*;

use be_block_protocol::OrganizationRole;

/// A demotion outdates every key made before it, as the organization's log has it, but an
/// administrator made after it still gets the settings' keys: key work seals a key to an
/// administrator the log names now, whatever epoch it is (signed access 3d).
#[tokio::test]
async fn an_administrator_made_after_a_demotion_opens_the_settings() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    let newcomer = harness.member(&admin, "newcomer@example.com").await;
    for (account, role) in [
        (deputy.account(), OrganizationRole::Administrator),
        (deputy.account(), OrganizationRole::Editor),
        (newcomer.account(), OrganizationRole::Administrator),
    ] {
        admin.set_role(account, role).await.unwrap();
    }
    admin.tend().await.unwrap();
    let settings = admin.organization_settings();
    let scope = admin.keys().scope_key(settings).unwrap();

    let device = harness.second(&newcomer).await;
    assert!(
        device.keys().has(scope),
        "an administrator made after a demotion cannot open the settings"
    );
    assert!(device.keys().escrow().is_some());
    assert_eq!(device.keys().escrow_warning(), None);

    harness.stop().await;
}
