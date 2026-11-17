use be_block_graph::Access;

use super::*;

/// A contact link's grant revoked before its maker's device finished the link: the revocation
/// signs nothing, since the block's log names no one yet, so finishing finds the server no
/// longer holds the grant and signs nothing either.
#[tokio::test]
async fn a_contact_link_revoked_before_it_is_finished_grants_nothing() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let team = workspace(&admin, "Team").await;
    let member = harness.member(&admin, "member@example.com").await;
    let card = crate::contacts::card(
        admin.account(),
        admin.keys().identity(),
        "Admin",
        "admin@example.com",
    );
    let offer = admin.offer(team, Access::Edit).await.unwrap();
    let link = crate::contacts::make_link(
        &admin.connection(),
        admin.keys().identity(),
        &card,
        Some(&offer),
    )
    .await
    .unwrap();
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
        &link,
        crate::contacts::Expected::Contact,
    )
    .await
    .unwrap();

    admin
        .grant(team, member.account(), Access::None)
        .await
        .unwrap();
    admin.finish_links().await.unwrap();
    let (_, checked) = admin.checked_access(team).await.unwrap();
    assert!(
        !checked
            .grants
            .get(&member.account().into_bytes())
            .is_some_and(|(level, _)| *level > be_block_keys::Level::None),
        "finishing the link granted the revoked access back"
    );
    assert!(
        !member
            .summary(team)
            .await
            .is_ok_and(|summary| summary.access.can_view()),
        "the revoked member views the block"
    );

    harness.stop().await;
}
