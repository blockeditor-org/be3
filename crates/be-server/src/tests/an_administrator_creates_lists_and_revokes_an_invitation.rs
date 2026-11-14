use super::*;

#[tokio::test]
async fn an_administrator_creates_lists_and_revokes_an_invitation() {
    let harness = Harness::start().await;
    let mut admin = harness.client().await;
    admin.register("admin@example.com").await;
    let organization = admin.organization("team").await;
    let mut invited = harness.member("invited@example.com").await;

    let link = admin
        .invite(organization, "Invited@Example.com")
        .await
        .expect("an administrator invites");
    let links = admin.links().await;
    assert!(
        links.iter().any(|own| own.id == link
            && own.accepter.is_none()
            && matches!(&own.kind, be_block_protocol::LinkKind::Invitation(invitation)
                if invitation.organization == organization
                    && invitation.email == "invited@example.com")),
        "an outstanding invitation is listed: {links:?}"
    );
    let found = invited
        .call(|peer| client::contacts::get_link(peer, link))
        .await;
    assert!(
        matches!(&found, Ok(found) if found.organization_name == "team"),
        "{found:?}"
    );

    let revoked = admin
        .call(|peer| client::contacts::delete_link(peer, link))
        .await;
    assert!(matches!(revoked, Ok(())), "{revoked:?}");
    assert!(admin.links().await.iter().all(|own| own.id != link));
    let found = invited
        .call(|peer| client::contacts::get_link(peer, link))
        .await;
    assert!(refused_as(&found, ErrorCode::LinkNotFound), "{found:?}");
    let accepted = invited
        .call(|peer| client::contacts::accept_link(peer, link, Vec::new(), Vec::new(), Vec::new()))
        .await;
    assert!(
        refused_as(&accepted, ErrorCode::LinkNotFound),
        "a revoked invitation is not accepted: {accepted:?}"
    );
    let joined = invited
        .call(|peer| client::organizations::list_organizations(peer))
        .await
        .expect("listing organizations failed");
    assert!(joined.iter().all(|joined| joined.id != organization));

    harness.stop().await;
}
