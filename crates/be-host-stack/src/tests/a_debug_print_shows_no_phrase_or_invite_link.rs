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
    let printed = format!("{view:?} {invite:?}");
    assert!(printed.contains("Ada"));
    for secret in ["abandon", "zebra", "SECRETCODE"] {
        assert!(
            !printed.contains(secret),
            "{secret} shows in a debug print: {printed}"
        );
    }
}
