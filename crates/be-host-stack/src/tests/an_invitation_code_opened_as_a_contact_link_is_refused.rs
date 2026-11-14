use super::*;

/// The Contacts dialog asks only "Connect with NAME?", so an invitation's code pasted there is
/// refused before anything is accepted: joining an organization is asked in its own dialog.
#[test]
fn an_invitation_code_opened_as_a_contact_link_is_refused() {
    let harness = Harness::start();
    let url = harness.url.clone();
    let guest = Someone::register(
        &harness.directory.join("server"),
        &url,
        "guest@example.com",
        "Guest",
    );
    harness.connect();
    let link = runtime()
        .block_on(crate::invite(
            "guest@example.com".to_owned(),
            be_block_protocol::OrganizationRole::Administrator,
        ))
        .expect("the owner invites the guest");
    let parsed = crate::invite_link::InviteLink::parse(&link).expect("the link parses");

    let connection = runtime()
        .block_on(crate::link::signed_in(
            &url,
            &guest.session.token,
            &be_block_client::Clock::real(),
        ))
        .expect("the guest signs in")
        .connection;
    let opened = runtime().block_on(be_block_client::contacts::open_link(
        &connection,
        &parsed.code,
        be_block_client::contacts::Expected::Contact,
    ));
    assert!(
        matches!(
            opened,
            Err(be_block_client::ClientError::Refused(
                be_block_protocol::ErrorCode::InvalidRequest,
                _
            ))
        ),
        "an invitation opened as a contact link: {opened:?}"
    );
    let accepted = runtime().block_on(be_block_client::contacts::accept_link(
        &connection,
        &guest.phrase.secret(),
        &guest.me().card(),
        &parsed.code,
        be_block_client::contacts::Expected::Contact,
    ));
    assert!(accepted.is_err(), "{accepted:?}");

    let still = runtime()
        .block_on(crate::accounts::invitation(
            url.clone(),
            guest.session.token.clone(),
            parsed.code.clone(),
        ))
        .expect("the invitation is still there, unused");
    assert_eq!(still.invitation.organization, harness.organization);
}
