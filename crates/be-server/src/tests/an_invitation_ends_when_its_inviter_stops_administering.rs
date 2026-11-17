use super::*;
use be_block_protocol::InvitationState;

#[tokio::test]
async fn an_invitation_ends_when_its_inviter_stops_administering() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.organization("team").await;
    // A second administrator, made by set-role after joining.
    let mut deputy = harness.member("deputy@example.com").await;
    let joined = owner.invite(organization).await.unwrap();
    assert!(matches!(deputy.accept(joined, &[]).await, Ok(())));
    let account = owner
        .call(|peer| peer.read::<Vec<be_block_protocol::Member>>(be_block_protocol::Query::Members))
        .await
        .unwrap()
        .into_iter()
        .find(|member| member.email == "deputy@example.com")
        .unwrap()
        .account;
    let promoted = owner
        .set_role(account, OrganizationRole::Administrator)
        .await;
    assert!(matches!(promoted, Ok(())), "{promoted:?}");
    deputy.open(organization).await;
    let link = deputy.invite(organization).await.unwrap();
    // Demoted, the deputy's outstanding invitation is revoked with it.
    let demoted = owner.set_role(account, OrganizationRole::Editor).await;
    assert!(matches!(demoted, Ok(())), "{demoted:?}");
    let mut invited = harness.member("invited@example.com").await;
    let found = invited
        .call(|peer| peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(link)))
        .await;
    assert!(
        matches!(&found, Err(error) if error.message.contains("revoked")),
        "{found:?}"
    );
    // Even were its row still open, it is refused where it is taken, and reads as revoked
    // with nothing else.
    metadata_database(&harness.directory)
        .execute(
            "UPDATE links SET invite_revoked = 0, invite_ended = 0 WHERE id = ?1",
            [link.to_string()],
        )
        .unwrap();
    let accepted = invited.accept(link, &[]).await;
    assert!(
        refused_as(&accepted, ErrorCode::LinkNotFound),
        "{accepted:?}"
    );
    let preview = harness
        .once(move |peer| {
            peer.read::<be_block_protocol::InvitationPreview>(be_block_protocol::Query::Invitation(
                link,
            ))
        })
        .await
        .unwrap();
    assert_eq!(preview.state, InvitationState::Revoked);
    assert!(preview.organization_name.is_empty() && preview.card.is_empty());
    let listed = owner
        .call(|peer| {
            peer.read_in::<Vec<be_block_protocol::InvitationRow>>(
                Some(organization),
                be_block_protocol::Query::Invitations,
            )
        })
        .await
        .unwrap();
    assert_eq!(
        listed
            .iter()
            .find(|row| row.id == link)
            .map(|row| row.state),
        Some(InvitationState::Revoked)
    );
    harness.stop().await;
}
