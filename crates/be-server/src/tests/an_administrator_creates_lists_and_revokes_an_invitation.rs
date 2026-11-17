use super::*;

#[tokio::test]
async fn an_administrator_creates_lists_and_revokes_an_invitation() {
    let harness = Harness::start().await;
    let mut admin = harness.client().await;
    admin.register("admin@example.com").await;
    let organization = admin.organization("team").await;
    let mut invited = harness.member("invited@example.com").await;

    let link = admin
        .invite(organization)
        .await
        .expect("an administrator invites");
    let links = admin.links().await;
    assert!(
        links.iter().any(|own| own.id == link
            && own.accepter.is_none()
            && matches!(&own.kind, be_block_protocol::LinkKind::Invitation(invitation)
                if invitation.organization == organization)),
        "an outstanding invitation is listed: {links:?}"
    );
    let found = invited
        .call(|peer| peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(link)))
        .await;
    assert!(
        matches!(&found, Ok(found) if found.organization_name == "team"),
        "{found:?}"
    );

    let revoked = admin
        .call(|peer| peer.write::<()>(be_block_protocol::Op::DeleteLink(link)))
        .await;
    assert!(matches!(revoked, Ok(())), "{revoked:?}");
    assert!(admin.links().await.iter().all(|own| own.id != link));
    let found = invited
        .call(|peer| peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(link)))
        .await;
    assert!(refused_as(&found, ErrorCode::LinkNotFound), "{found:?}");
    let accepted = invited
        .call(|peer| peer.write::<()>(be_block_protocol::Op::AcceptLink { link: link, card: Vec::new(), contact: Vec::new(), key: Vec::new() }))
        .await;
    assert!(
        refused_as(&accepted, ErrorCode::LinkNotFound),
        "a revoked invitation is not accepted: {accepted:?}"
    );
    let joined = invited
        .call(|peer| peer.read::<Vec<be_block_protocol::Organization>>(be_block_protocol::Query::Organizations))
        .await
        .expect("listing organizations failed");
    assert!(joined.iter().all(|joined| joined.id != organization));

    harness.stop().await;
}
