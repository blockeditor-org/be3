use super::*;

/// Whether `client` lists `account` among its contacts.
async fn knows(client: &mut TestClient, account: Uuid) -> bool {
    client
        .contacts()
        .await
        .iter()
        .any(|contact| contact.account == account)
}

#[tokio::test]
async fn accepting_either_kind_of_link_makes_two_contacts() {
    let harness = Harness::start().await;
    let mut admin = harness.client().await;
    let admin_id = admin.register("admin@example.com").await;
    let organization = admin.organization("team").await;

    let mut friend = harness.client().await;
    let friend_id = friend.register("friend@example.com").await;
    let link = admin.link(None).await;
    let accepted = friend.accept(link, &[]).await;
    assert!(matches!(accepted, Ok(())), "{accepted:?}");
    assert!(knows(&mut friend, admin_id).await);
    assert!(knows(&mut admin, friend_id).await);

    let mut colleague = harness.client().await;
    let colleague_id = colleague.register("colleague@example.com").await;
    let invitation = admin
        .invite(organization, "colleague@example.com")
        .await
        .expect("an administrator invites");
    let accepted = colleague.accept(invitation, &[]).await;
    assert!(matches!(accepted, Ok(())), "{accepted:?}");
    assert!(knows(&mut colleague, admin_id).await);
    assert!(knows(&mut admin, colleague_id).await);
    colleague.open(organization).await;

    for (link, accepter) in [(link, friend_id), (invitation, colleague_id)] {
        let links = admin.links().await;
        assert!(
            links
                .iter()
                .any(|own| own.id == link && own.accepter == Some(accepter)),
            "the accepted link waits for its owner to finish it: {links:?}"
        );
    }

    harness.stop().await;
}
