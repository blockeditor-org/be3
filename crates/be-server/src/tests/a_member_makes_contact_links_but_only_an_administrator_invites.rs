use super::*;

#[tokio::test]
async fn a_member_makes_contact_links_but_only_an_administrator_invites() {
    let harness = Harness::start().await;
    let mut admin = harness.client().await;
    admin.register("admin@example.com").await;
    let organization = admin.organization("team").await;
    let mut editor = harness.member("editor@example.com").await;
    let invitation = admin
        .invite(organization, "editor@example.com")
        .await
        .expect("an administrator invites");
    let joined = editor
        .call(|peer| {
            client::contacts::accept_link(peer, invitation, Vec::new(), Vec::new(), Vec::new())
        })
        .await;
    assert!(matches!(joined, Ok(())), "{joined:?}");
    editor.open(organization).await;

    let refused = editor.invite(organization, "friend@example.com").await;
    assert!(
        refused_as(&refused, ErrorCode::PermissionDenied),
        "an editor invited someone: {refused:?}"
    );
    let link = Uuid::new_v4();
    let made = editor
        .create_link(link, be_block_protocol::LinkKind::Contact(None))
        .await;
    assert!(matches!(made, Ok(())), "an editor makes a contact link: {made:?}");
    assert!(editor.links().await.iter().any(|own| own.id == link));

    harness.stop().await;
}
