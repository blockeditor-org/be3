use super::*;

async fn invite(admin: &mut TestClient, id: Uuid, organization: Uuid, email: &str) -> Result<(), Error> {
    admin
        .call(|peer| {
            client::organizations::invite(
                peer,
                id,
                organization,
                email.into(),
                OrganizationRole::Editor,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        })
        .await
}

/// An invitation becomes a contact link under its own id once accepted, so an administrator who
/// picks the id of someone else's link must not overwrite it.
#[tokio::test]
async fn an_invitation_never_takes_the_id_of_a_contact_link() {
    let harness = Harness::start().await;
    let mut victim = harness.member("victim@example.com").await;
    let mut attacker = harness.member("attacker@example.com").await;
    let organization = attacker.organization("lure").await;
    let mut accomplice = harness.member("accomplice@example.com").await;

    let link = victim.link(None).await;
    let invited = invite(&mut attacker, link, organization, "accomplice@example.com").await;
    assert!(refused_as(&invited, ErrorCode::InvalidRequest), "{invited:?}");

    let early = Uuid::new_v4();
    let invited = invite(&mut attacker, early, organization, "accomplice@example.com").await;
    assert!(matches!(invited, Ok(())), "{invited:?}");
    let response = victim
        .call(|peer| {
            client::contacts::create_contact_link(
                peer,
                early,
                b"the victim's card".to_vec(),
                b"the victim's code".to_vec(),
                None,
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let accepted = accomplice
        .call(|peer| {
            client::organizations::accept_invitation(peer, early, Vec::new(), Vec::new(), Vec::new())
        })
        .await;
    assert!(refused_as(&accepted, ErrorCode::InvalidRequest), "{accepted:?}");

    let links = victim
        .call(|peer| client::contacts::list_contact_links(peer))
        .await
        .expect("listing links failed");
    for id in [link, early] {
        let kept = links.iter().find(|own| own.id == id);
        assert!(
            kept.is_some_and(|own| own.accepter.is_none() && !own.code.is_empty()),
            "the victim's link {id} changed: {links:?}"
        );
    }
    let joined = accomplice
        .call(|peer| client::organizations::list_organizations(peer))
        .await
        .expect("listing organizations failed");
    assert!(
        joined.iter().all(|joined| joined.id != organization),
        "a refused acceptance still joined the organization"
    );

    harness.stop().await;
}
