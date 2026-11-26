use super::*;

/// A recovery phrase on its way to the screen and an invitation's link, a bearer secret, print
/// without them, as any `Debug` might end up in a log.
#[test]
fn a_debug_print_shows_no_phrase_or_invite_link() {
    let view = crate::keys::RecoveryView {
        words: vec!["abandon".into(), "zebra".into()],
        ..Default::default()
    };
    let invite = crate::Invite {
        id: Uuid::nil(),
        inviter: "Ada".into(),
        link: Some("https://be.example/#invite/SECRETCODE".into()),
        state: be_block_protocol::InvitationState::Waiting,
        accepter: String::new(),
        created: 0,
        expires: 0,
        opened: 0,
        ended: 0,
    };
    let session = crate::accounts::Session {
        account: Uuid::nil(),
        email: "ada@example.com".into(),
        display_name: "Ada".into(),
        token: "SESSIONTOKEN".into(),
    };
    let agent = crate::accounts::AgentCredentials {
        name: "Helper".into(),
        server: "https://be.example".into(),
        organization: Uuid::nil(),
        token: "AGENTTOKEN".into(),
        phrase: "agentphrase".into(),
    };
    let unlock = crate::keys::UnlockView {
        account: "Ada".into(),
        code: Some("PAIRCODE".into()),
        error: None,
    };
    let printed = format!("{view:?} {invite:?} {session:?} {agent:?} {unlock:?}");
    assert!(printed.contains("Ada"));
    for secret in [
        "abandon",
        "zebra",
        "SECRETCODE",
        "SESSIONTOKEN",
        "AGENTTOKEN",
        "agentphrase",
        "PAIRCODE",
    ] {
        assert!(
            !printed.contains(secret),
            "{secret} shows in a debug print: {printed}"
        );
    }
}
