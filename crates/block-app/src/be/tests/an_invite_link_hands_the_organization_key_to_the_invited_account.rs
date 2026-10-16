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
fn an_invite_link_hands_the_organization_key_to_the_invited_account() {
    let harness = Harness::start();
    let url = harness.url.clone();
    let guest = register(&url, "guest@example.com");
    let outsider = register(&url, "outsider@example.com");
    let phrase = be_keys::RecoveryPhrase::generate();
    runtime()
        .block_on(crate::accounts::set_recovery_key(
            url.clone(),
            guest.token.clone(),
            phrase.secret().public(),
            Vec::new(),
        ))
        .expect("the guest saves a recovery phrase");
    let link = runtime()
        .block_on(crate::accounts::invite(
            url.clone(),
            harness.token.clone(),
            harness.organization,
            "guest@example.com".to_owned(),
            be_protocol::OrganizationRole::Editor,
            harness.content_key,
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
    assert!(
        !invitation
            .sealed
            .windows(harness.content_key.len())
            .any(|window| window == harness.content_key),
        "the invitation carries the organization key in the clear"
    );

    let join = |token: &str| {
        runtime().block_on(crate::accounts::accept_invitation(
            url.clone(),
            token.to_owned(),
            parsed.code.clone(),
            Some(phrase.secret().public()),
        ))
    };
    assert!(
        join(&outsider.token).is_err(),
        "an account with another email joined with the link"
    );
    assert_eq!(
        join(&guest.token).expect("the guest joins with the link"),
        (harness.organization, harness.content_key)
    );
    assert!(join(&guest.token).is_err(), "the link worked twice");

    let keys = runtime()
        .block_on(crate::accounts::keys(url, guest.token.clone()))
        .expect("the guest's keys load");
    assert_eq!(
        phrase
            .secret()
            .open(&keys.sealed[&harness.organization])
            .unwrap(),
        harness.content_key,
        "the guest did not seal the key to their own recovery phrase"
    );
}
