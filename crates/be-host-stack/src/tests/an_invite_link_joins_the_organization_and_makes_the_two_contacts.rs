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
        .block_on(crate::invite())
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
    assert_eq!(invitation.invitation.organization, harness.organization);
    let preview = runtime()
        .block_on(crate::accounts::preview_invitation(
            url.clone(),
            parsed.code.clone(),
        ))
        .expect("the invitation reads before signing in");
    assert_eq!(preview.organization_name, invitation.organization_name);
    assert_eq!(preview.inviter, invitation.inviter);

    let join = |someone: &Someone| {
        runtime().block_on(crate::accounts::accept_invitation(
            url.clone(),
            someone.session.token.clone(),
            someone.me(),
            parsed.code.clone(),
        ))
    };
    // The link is bound to no email: whoever opens it first joins, once.
    assert_eq!(
        join(&guest).expect("the guest joins with the link"),
        harness.organization
    );
    assert!(join(&guest).is_err(), "the link worked twice");
    assert!(
        join(&outsider).is_err(),
        "the link worked again for another account"
    );

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
        .block_on(be_block_client::contacts::contacts(
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
