use super::*;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}

#[test]
fn an_invite_code_hands_the_workspace_key_to_the_invited_account() {
    let harness = Harness::start();
    let url = harness.url.clone();
    let guest = runtime()
        .block_on(crate::accounts::register(
            url.clone(),
            "guest@example.com".to_owned(),
            "Guest".to_owned(),
            "correct horse battery".to_owned(),
        ))
        .expect("the guest registers");
    let phrase = be_keys::RecoveryPhrase::generate();
    runtime()
        .block_on(crate::accounts::set_recovery_key(
            url.clone(),
            guest.token.clone(),
            phrase.secret().public(),
            Vec::new(),
        ))
        .expect("the guest saves a recovery phrase");
    let code = runtime()
        .block_on(crate::accounts::invite(
            url.clone(),
            harness.token.clone(),
            harness.workspace,
            "guest@example.com".to_owned(),
            be_protocol::WorkspaceRole::Editor,
            harness.content_key,
        ))
        .expect("the owner invites the guest");
    let (_, invitations) = runtime()
        .block_on(crate::accounts::workspaces(
            url.clone(),
            guest.token.clone(),
        ))
        .expect("the guest lists invitations");
    let invitation = invitations[0].id;
    assert!(
        !invitations[0]
            .sealed
            .windows(harness.content_key.len())
            .any(|window| window == harness.content_key),
        "the invitation carries the workspace key in the clear"
    );

    let accept = |code: String| {
        runtime().block_on(crate::accounts::accept_invitation(
            url.clone(),
            guest.token.clone(),
            invitation,
            code,
            Some(phrase.secret().public()),
        ))
    };
    assert!(
        accept(be_keys::InviteCode::generate().to_string()).is_err(),
        "another code accepted the invitation"
    );
    let (_, invitations) = runtime()
        .block_on(crate::accounts::workspaces(
            url.clone(),
            guest.token.clone(),
        ))
        .expect("the guest lists invitations again");
    assert_eq!(invitations.len(), 1, "a wrong code used up the invitation");

    assert_eq!(
        accept(code.to_lowercase()).expect("the right code accepts"),
        (harness.workspace, harness.content_key)
    );
    let keys = runtime()
        .block_on(crate::accounts::keys(url, guest.token.clone()))
        .expect("the guest's keys load");
    assert_eq!(
        phrase
            .secret()
            .open(&keys.sealed[&harness.workspace])
            .unwrap(),
        harness.content_key,
        "the guest did not seal the key to their own recovery phrase"
    );
}
