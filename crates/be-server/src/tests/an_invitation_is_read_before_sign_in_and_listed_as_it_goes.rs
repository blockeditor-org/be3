use super::*;
use be_block_protocol::InvitationState;

#[tokio::test]
async fn an_invitation_is_read_before_sign_in_and_listed_as_it_goes() {
    let harness = Harness::start().await;
    let mut admin = harness.client().await;
    admin.register("admin@example.com").await;
    let organization = admin.organization("team").await;
    let link = Uuid::new_v4();
    let made = admin
        .call(|peer| {
            client::contacts::create_link(
                peer,
                link,
                b"the inviter's sealed card".to_vec(),
                b"the code sealed to the inviter".to_vec(),
                be_block_protocol::LinkKind::Invitation(be_block_protocol::Invitation {
                    organization,
                    escrow: b"the sealed escrow key".to_vec(),
                }),
            )
        })
        .await;
    assert!(matches!(made, Ok(())), "{made:?}");
    let listed = |rows: &[be_block_protocol::InvitationRow]| {
        rows.iter().find(|row| row.id == link).cloned().unwrap()
    };
    let row = listed(
        &admin
            .call(|peer| client::contacts::list_invitations(peer, organization))
            .await
            .unwrap(),
    );
    assert_eq!(row.state, InvitationState::Waiting);
    assert_eq!(row.code, b"the code sealed to the inviter");
    assert_eq!(
        row.expires - row.created,
        crate::limits::INVITATION_LIFETIME * 1000
    );
    // Read before signing in, a one-shot call: the organization's name and the sealed card,
    // and nothing else, and the inviter then sees it opened.
    let preview = harness
        .once(move |peer| client::contacts::read_invitation(peer, link))
        .await
        .expect("an invitation is read before sign-in");
    assert_eq!(preview.organization_name, "team");
    assert_eq!(preview.card, b"the inviter's sealed card");
    assert_eq!(preview.state, InvitationState::Opened);
    let row = listed(
        &admin
            .call(|peer| client::contacts::list_invitations(peer, organization))
            .await
            .unwrap(),
    );
    assert_eq!(row.state, InvitationState::Opened);
    assert!(row.opened > 0);
    // A contact link, or a guess, reads as no link.
    let contact = admin.link(None).await;
    for id in [contact, Uuid::new_v4()] {
        let read = harness
            .once(move |peer| client::contacts::read_invitation(peer, id))
            .await;
        assert!(refused_as(&read, ErrorCode::LinkNotFound), "{read:?}");
    }
    // Accepted, it shows who accepted it; another administrator sees it without its code, and
    // an editor not at all.
    let mut joiner = harness.member("joiner@example.com").await;
    let accepted = joiner
        .call(|peer| client::contacts::accept_link(peer, link, Vec::new(), Vec::new(), Vec::new()))
        .await;
    assert!(matches!(accepted, Ok(())), "{accepted:?}");
    let row = listed(
        &admin
            .call(|peer| client::contacts::list_invitations(peer, organization))
            .await
            .unwrap(),
    );
    assert_eq!(row.state, InvitationState::Accepted);
    assert_eq!(row.accepter_name, "joiner@example.com");
    assert!(row.ended > 0);
    let preview = harness
        .once(move |peer| client::contacts::read_invitation(peer, link))
        .await
        .unwrap();
    assert_eq!(preview.state, InvitationState::Accepted);
    joiner.open(organization).await;
    let listing = joiner
        .call(|peer| client::contacts::list_invitations(peer, organization))
        .await;
    assert!(
        refused_as(&listing, ErrorCode::PermissionDenied),
        "{listing:?}"
    );
    harness.stop().await;
}
