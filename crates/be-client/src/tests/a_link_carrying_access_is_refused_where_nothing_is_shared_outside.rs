use be_block::{Allowance, OrganizationPolicies};
use be_protocol::ErrorCode;

use super::an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server::set_policies;
use super::*;

#[tokio::test]
async fn a_link_carrying_access_is_refused_where_nothing_is_shared_outside() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let team = workspace(&admin, "Team").await;
    let card = crate::contacts::card(
        admin.account(),
        admin.keys().identity(),
        "Admin",
        "admin@example.com",
    );
    let offer = admin.offer(team, be_graph::Access::View).await.unwrap();
    let early = crate::contacts::make_link(
        &admin.connection(),
        admin.keys().identity(),
        &card,
        Some(&offer),
    )
    .await
    .expect("a link carries access while the organization allows it");

    set_policies(
        &admin,
        &[OrganizationPolicies::set_outside_sharing(Some(
            Allowance::Blocked,
        ))],
    )
    .await;
    let refused = crate::contacts::make_link(
        &admin.connection(),
        admin.keys().identity(),
        &card,
        Some(&offer),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(refused, ClientError::Refused(ErrorCode::PolicyRefused, _)),
        "{refused}"
    );
    crate::contacts::make_link(&admin.connection(), admin.keys().identity(), &card, None)
        .await
        .expect("a link that only makes a contact is still made");

    let member = harness.member(&admin, "member@example.com").await;
    let member_card = crate::contacts::card(
        member.account(),
        member.keys().identity(),
        "Member",
        "member@example.com",
    );
    crate::contacts::accept_link(
        &member.connection(),
        member.keys().identity(),
        &member_card,
        &early,
    )
    .await
    .expect("the link still makes a contact");
    assert!(
        !member
            .summary(team)
            .await
            .is_ok_and(|summary| summary.access.can_view()),
        "a link made before sharing outside was blocked still granted access"
    );

    harness.stop().await;
}
