use be_block_graph::Access;

use super::*;

/// A device that finished a contact link's grant and stopped before deleting the link finishes
/// it again on its next sync: by then the block's log names the accepter, so a revocation made
/// in between stands.
#[tokio::test]
async fn finishing_a_contact_link_again_does_not_undo_a_revocation() {
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
    let named = || async {
        let (_, checked) = admin.checked_access(team).await.unwrap();
        checked
            .grants
            .get(&member.account().into_bytes())
            .map(|(level, _)| *level)
    };

    // Finished, but stopped before the delete.
    let opened = crate::contacts::finish_links(
        &admin.connection(),
        admin.keys().identity(),
        admin.organization(),
    )
    .await
    .unwrap();
    assert_eq!(opened.len(), 1, "{opened:?}");
    admin.finish_link(&opened[0]).await.unwrap();
    assert_eq!(named().await, Some(be_block_keys::Level::Edit));

    admin
        .grant(team, member.account(), Access::None)
        .await
        .unwrap();
    admin.finish_links().await.unwrap();
    assert_eq!(
        named().await,
        Some(be_block_keys::Level::None),
        "finishing the link again granted the access back"
    );
    let entries = admin.checked_access(team).await.unwrap().0;
    assert_eq!(
        entries
            .iter()
            .filter(|entry| matches!(
                &entry.record,
                be_block_keys::Record::Grant(grant) if grant.grantee == member.account().into_bytes()
            ))
            .count(),
        2,
        "the log holds more than the link's grant and the revocation"
    );
    assert!(
        !member
            .summary(team)
            .await
            .is_ok_and(|summary| summary.access.can_view()),
        "the revoked member views the block again"
    );

    harness.stop().await;
}
