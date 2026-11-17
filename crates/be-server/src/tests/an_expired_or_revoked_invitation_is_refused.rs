use super::*;
use be_block_protocol::InvitationState;

#[tokio::test]
async fn an_expired_or_revoked_invitation_is_refused() {
    let harness = Harness::start().await;
    let mut admin = harness.member("admin@example.com").await;
    let organization = admin.organization("team").await;
    let expiring = admin.invite(organization).await.unwrap();
    let revoked = admin.invite(organization).await.unwrap();
    // Seven days pass for the first.
    metadata_database(&harness.directory)
        .execute(
            "UPDATE links SET invite_expires = unixepoch() - 1 WHERE id = ?1",
            [expiring.to_string()],
        )
        .unwrap();
    let revoking = admin
        .call(|peer| peer.write::<()>(be_block_protocol::Op::DeleteLink(revoked)))
        .await;
    assert!(matches!(revoking, Ok(())), "{revoking:?}");
    let mut invited = harness.member("invited@example.com").await;
    for (link, why) in [(expiring, "expired"), (revoked, "revoked")] {
        let found = invited
            .call(|peer| peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(link)))
            .await;
        assert!(
            matches!(&found, Err(error) if error.message.contains(why)),
            "{why}: {found:?}"
        );
        let accepted = invited
            .call(|peer| {
                peer.write::<()>(be_block_protocol::Op::AcceptLink {
                    link: link,
                    card: Vec::new(),
                    contact: Vec::new(),
                    key: Vec::new(),
                })
            })
            .await;
        assert!(
            refused_as(&accepted, ErrorCode::LinkNotFound),
            "{why}: {accepted:?}"
        );
    }
    // Both stay in the list, saying so, and no longer count as outstanding links.
    let listed = admin
        .call(|peer| {
            peer.read_in::<Vec<be_block_protocol::InvitationRow>>(
                Some(organization),
                be_block_protocol::Query::Invitations,
            )
        })
        .await
        .unwrap();
    let state = |link: Uuid| {
        listed
            .iter()
            .find(|row| row.id == link)
            .map(|row| row.state)
    };
    assert_eq!(state(expiring), Some(InvitationState::Expired));
    assert_eq!(state(revoked), Some(InvitationState::Revoked));
    assert!(admin.links().await.is_empty());
    // One accepted that its inviter's device has not finished yet stays past its retention:
    // the accepter is not their contact until it is.
    let accepted = admin.invite(organization).await.unwrap();
    assert!(matches!(invited.accept(accepted, &[]).await, Ok(())));
    metadata_database(&harness.directory)
        .execute(
            "UPDATE links SET invite_ended = 1 WHERE id = ?1",
            [accepted.to_string()],
        )
        .unwrap();
    // Once their retention passes, the others go.
    metadata_database(&harness.directory)
        .execute(
            "UPDATE links SET invite_ended = 1 WHERE invite_revoked = 1;",
            [],
        )
        .unwrap();
    metadata_database(&harness.directory)
        .execute(
            "UPDATE links SET invite_expires = 1 WHERE invite_revoked = 0",
            [],
        )
        .unwrap();
    let listed = admin
        .call(|peer| {
            peer.read_in::<Vec<be_block_protocol::InvitationRow>>(
                Some(organization),
                be_block_protocol::Query::Invitations,
            )
        })
        .await
        .unwrap();
    assert_eq!(
        listed.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![accepted],
        "only the accepted one not yet finished stays"
    );
    harness.stop().await;
}
