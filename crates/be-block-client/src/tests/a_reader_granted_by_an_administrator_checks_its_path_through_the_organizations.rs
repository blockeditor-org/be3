use super::*;

use be_block_graph::Access;
use be_block_keys::{Authority, Record, Step};

/// A member's block, which an administrator who holds nothing in its log grants a reader View
/// on, naming their Manage as administering the organization (`Authority::Organization`): the
/// reader's proof path of the block's log carries that grant whole, and checks it against its
/// proof path of the organization's log, whose administrator records are all whole
/// (plans/signed_access.md, 3e). No other member's grant reaches it.
#[tokio::test]
async fn a_reader_granted_by_an_administrator_checks_its_path_through_the_organizations() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let maker = harness.member(&admin, "maker@example.com").await;
    let other = harness.member(&admin, "other@example.com").await;
    let vera = harness.member(&admin, "vera@example.com").await;
    befriend(&maker, &vera).await;
    befriend(&maker, &other).await;
    let team = workspace(&maker, "Team").await;
    maker
        .grant(team, other.account(), Access::Edit)
        .await
        .unwrap();
    maker.tend().await.unwrap();
    admin.refresh_keys(None).await.unwrap();
    admin
        .grant(team, vera.account(), Access::View)
        .await
        .unwrap();
    maker.tend().await.unwrap();
    vera.refresh_keys(None).await.unwrap();

    let (steps, checked) = vera.checked_access(team).await.unwrap();
    assert!(checked.partial);
    let grants: Vec<_> = steps
        .iter()
        .filter_map(Step::entry)
        .filter_map(|entry| match &entry.record {
            Record::Grant(grant) => Some((grant.grantee, grant.authority.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        grants.len(),
        1,
        "the reader is served another member's grant"
    );
    assert_eq!(grants[0].0, vera.account().into_bytes());
    assert!(
        matches!(grants[0].1, Authority::Organization(_)),
        "the administrator granted from the log itself"
    );
    assert_eq!(
        checked
            .grants
            .get(&vera.account().into_bytes())
            .map(|(level, _)| *level),
        Some(be_block_keys::Level::View)
    );
}
