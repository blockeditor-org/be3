use super::*;

#[test]
fn an_invite_link_joins_the_organization_and_makes_the_two_contacts() {
    let harness = Harness::start();
    let url = harness.url.clone();
    let guest = Someone::register(
        &harness.directory.join("server"),
        &url,
        "guest@example.com",
        "Guest",
    );
    let outsider = Someone::register(
        &harness.directory.join("server"),
        &url,
        "outsider@example.com",
        "Outsider",
    );
    harness.connect();
    let link = runtime()
        .block_on(crate::invite(
            "guest@example.com".to_owned(),
            be_protocol::OrganizationRole::Editor,
        ))
        .expect("the owner invites the guest");
    let parsed = crate::invite_link::InviteLink::parse(&link).expect("the link parses");
    assert!(parsed.is_for(&crate::accounts::link_base(&url)), "{link}");

    let invitation = runtime()
        .block_on(crate::accounts::invitation(
            url.clone(),
            guest.session.token.clone(),
            parsed.code.clone(),
        ))
        .expect("the guest reads the invitation");
    assert_eq!(invitation.organization, harness.organization);

    let join = |someone: &Someone| {
        runtime().block_on(crate::accounts::accept_invitation(
            url.clone(),
            someone.session.token.clone(),
            someone.me(),
            parsed.code.clone(),
        ))
    };
    assert!(
        join(&outsider).is_err(),
        "an account with another email joined with the link"
    );
    assert_eq!(
        join(&guest).expect("the guest joins with the link"),
        harness.organization
    );
    assert!(join(&guest).is_err(), "the link worked twice");

    harness.connect();
    wait_until("confirmed the invited account as a contact", |shared| {
        shared.contacts.iter().any(|contact| {
            contact.account == guest.session.account
                && contact.public == Some(guest.phrase.secret().public())
        })
    });
    let runtime = runtime();
    let peer = runtime.block_on(guest.peer(&url, harness.organization));
    let theirs = runtime
        .block_on(be_client::contacts::contacts(
            &peer.connection(),
            peer.account(),
            &guest.phrase.secret(),
        ))
        .expect("the guest lists its contacts");
    assert_eq!(
        theirs
            .iter()
            .map(|contact| (contact.account, contact.public))
            .collect::<Vec<_>>(),
        vec![(harness.account, Some(harness.phrase.secret().public()))]
    );
}
