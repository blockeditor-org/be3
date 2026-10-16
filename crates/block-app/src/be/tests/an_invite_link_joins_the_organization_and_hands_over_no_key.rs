use super::*;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}

fn register(url: &str, email: &str) -> crate::accounts::Session {
    runtime()
        .block_on(crate::accounts::register(
            url.to_owned(),
            email.to_owned(),
            "Someone".to_owned(),
            "correct horse battery".to_owned(),
        ))
        .expect("the account registers")
}

#[test]
fn an_invite_link_joins_the_organization_and_hands_over_no_key() {
    let harness = Harness::start();
    let url = harness.url.clone();
    let guest = register(&url, "guest@example.com");
    let outsider = register(&url, "outsider@example.com");
    let link = runtime()
        .block_on(crate::accounts::invite(
            url.clone(),
            harness.token.clone(),
            harness.organization,
            "guest@example.com".to_owned(),
            be_protocol::OrganizationRole::Editor,
        ))
        .expect("the owner invites the guest");
    let parsed = crate::invite_link::InviteLink::parse(&link).expect("the link parses");
    assert!(parsed.is_for(&crate::accounts::link_base(&url)), "{link}");

    let invitation = runtime()
        .block_on(crate::accounts::invitation(
            url.clone(),
            guest.token.clone(),
            parsed.invitation(),
        ))
        .expect("the guest reads the invitation");
    assert_eq!(invitation.organization, harness.organization);
    assert!(invitation.sealed.is_empty(), "the invitation carries a key");

    let join = |token: &str| {
        runtime().block_on(crate::accounts::accept_invitation(
            url.clone(),
            token.to_owned(),
            parsed.code.clone(),
        ))
    };
    assert!(
        join(&outsider.token).is_err(),
        "an account with another email joined with the link"
    );
    assert_eq!(
        join(&guest.token).expect("the guest joins with the link"),
        harness.organization
    );
    assert!(join(&guest.token).is_err(), "the link worked twice");

    let organizations = runtime()
        .block_on(crate::accounts::organizations(url, guest.token.clone()))
        .expect("the guest's organizations load");
    assert!(
        organizations
            .iter()
            .any(|organization| organization.id == harness.organization)
    );
    assert!(
        organizations
            .iter()
            .any(|organization| organization.kind == be_protocol::OrganizationKind::Personal),
        "registering did not join the personal organization"
    );
}
