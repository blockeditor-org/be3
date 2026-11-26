use super::*;

use be_block_keys::{Record, Step};

/// An account removed from an organization still signed in its log, so a new phrase retires its
/// old key there too. It may no longer read the log as a member, so the `recovery-key` answer
/// carries the log from its root as the account's proof path, from which its device learns it
/// does not administer the organization (`Retirement::administrator`).
#[tokio::test]
async fn an_account_removed_from_an_organization_takes_a_new_phrase() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let removed = harness.member(&owner, "removed@example.com").await;
    let organization = owner.organization();
    let old = removed.keys().identity().clone();
    owner.remove_member(removed.account()).await.unwrap();

    // The removal closed its connection to the organization; it signs in again.
    let (connection, _) = Connection::connect(
        &harness.url,
        &Clock::real(),
        &Credentials::Token(removed.token()),
    )
    .await
    .expect("the server signs the removed account in");
    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    take_phrase(&connection, removed.account(), &old, &fresh).await;

    let log = owner.organization_log().await.unwrap();
    let Some(Step::Entry(last)) = log.last() else {
        panic!("the owner is served the organization's log whole: {log:?}");
    };
    assert!(
        matches!(
            &last.record,
            Record::Retirement(retirement)
                if retirement.account == removed.account().into_bytes() && !retirement.administrator
        ),
        "the organization's log ends in the removed member's retirement, of a member: {last:?}"
    );
    assert_eq!(last.log, organization.into_bytes());

    harness.stop().await;
}
