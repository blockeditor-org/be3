use be_block::{Allowance, OrganizationPolicies};
use be_block_keys::Admits;
use be_block_protocol::ErrorCode;

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

/// The policy is checked again as a link opens: a link that admits one, made while the
/// organization shared outside, is refused once it blocks sharing outside.
#[tokio::test]
async fn a_link_for_one_made_while_allowed_is_refused_once_nothing_is_shared_outside() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let invited = harness.owner("invited@example.com").await;
    let team = workspace(&admin, "Team").await;
    let code = admin
        .make_share_link(team, Access::View, Admits::One)
        .await
        .unwrap();

    set_policies(
        &admin,
        &[OrganizationPolicies::set_outside_sharing(Some(
            Allowance::Blocked,
        ))],
    )
    .await;
    let refused = crate::share_links::open_share_link(
        &invited.connection(),
        invited.keys().identity(),
        invited.account(),
        &code,
        None,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(refused, ClientError::Refused(ErrorCode::PolicyRefused, _)),
        "{refused}"
    );

    harness.stop().await;
}
