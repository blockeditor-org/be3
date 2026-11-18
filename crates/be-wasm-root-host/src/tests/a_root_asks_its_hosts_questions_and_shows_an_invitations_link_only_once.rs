use be_lib_geometry::pos2;
use be_lib_ui_types::{Key, Modifiers};
use be_sdk_root_testing::{DIALOG_LAYER, DIALOG_SURFACE, OPENS_INVITATIONS};
use be_wasm_root_api::{
    BrowserSignIn, CoreCommand, CoreSecret, CoreView, DiscardReason, InvitationRow,
    InvitationState, InvitationsArea, Reauthenticating, SecretCommand, SigningIn,
};

use super::*;

const MINE: [u8; 16] = [8; 16];
const LINK: &str = "https://example.com/invite#a-secret";

/// Hands the root `key` and draws a few frames, answering what it asked of the block stack.
fn press(root: &mut Root, time: &mut u64, key: Key) -> Vec<CoreCommand> {
    root.input(None, be_lib_ui_types::input::encode(&chord(key, Modifiers::NONE)))
        .expect("the root takes input");
    settle(root, time)
}

/// Draws a few frames, answering what the root asked of the block stack.
fn settle(root: &mut Root, time: &mut u64) -> Vec<CoreCommand> {
    for _ in 0..4 {
        *time += 1;
        step(root, *time);
    }
    root.take_core_commands()
}

/// Draws frames until the root asks for a secret, answering the one request.
fn requested(root: &mut Root, time: &mut u64) -> (u64, SecretCommand) {
    let asked: Vec<_> = (0..6)
        .flat_map(|_| {
            *time += 1;
            step(root, *time);
            root.take_secret_requests()
        })
        .collect();
    match <[_; 1]>::try_from(asked) {
        Ok([asked]) => asked,
        Err(asked) => panic!("the root asks for one secret: {asked:?}"),
    }
}

/// Whether the root shows its dialog now.
fn shows_dialog(root: &mut Root, time: &mut u64) -> bool {
    *time += 1;
    layer_of(&step(root, *time), DIALOG_LAYER).is_some()
}

/// What the root paints its dialog with, a few frames from `*time` on.
fn dialog_painting(root: &mut Root, time: &mut u64) -> beui_paint_format::Painting {
    let _ = settle(root, time);
    own_paintings(root, *time)
        .into_iter()
        .find(|(id, _)| *id == u64::from(DIALOG_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its dialog")
}

fn discarding(reason: Option<DiscardReason>) -> CoreView {
    CoreView {
        discarding: reason,
        ..CoreView::default()
    }
}

fn reauthenticating(signing_in: Option<SigningIn>) -> CoreView {
    CoreView {
        reauthenticating: Some(Reauthenticating {
            key: "https://example.com\u{0}grace".to_owned(),
            name: "Grace".to_owned(),
            busy: signing_in.is_some(),
            error: None,
        }),
        signing_in,
        ..CoreView::default()
    }
}

fn invitations() -> CoreView {
    CoreView {
        invitations: InvitationsArea {
            listed: true,
            rows: vec![
                InvitationRow {
                    id: MINE,
                    state: InvitationState::Waiting,
                    inviter: "Ada".to_owned(),
                    accepter: None,
                    created: 1_763_000_000_000,
                    opened: None,
                    expires: 1_763_600_000_000,
                    ended: None,
                    has_link: true,
                },
                InvitationRow {
                    id: [9; 16],
                    state: InvitationState::Accepted,
                    inviter: "Ada".to_owned(),
                    accepter: Some("Grace".to_owned()),
                    created: 1_762_000_000_000,
                    opened: Some(1_762_100_000_000),
                    expires: 1_762_600_000_000,
                    ended: Some(1_762_200_000_000),
                    has_link: false,
                },
            ],
            busy: false,
            error: None,
        },
        ..CoreView::default()
    }
}

#[test]
fn a_root_asks_its_hosts_questions_and_shows_an_invitations_link_only_once() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);

    // The host asks before discarding unsaved changes: Keep, or Escape, keeps them.
    root.show_core(discarding(Some(DiscardReason::SwitchOrganization)));
    beui_paint_snapshot::assert_painting(
        "a_root_asks_before_its_host_discards_unsaved_changes",
        dialog_painting(&mut root, &mut time),
    );
    let commands = press(&mut root, &mut time, Key::Escape);
    assert!(
        matches!(commands.as_slice(), [CoreCommand::CancelDiscard]),
        "Escape keeps the changes: {commands:?}"
    );
    root.show_core(discarding(None));
    assert!(!shows_dialog(&mut root, &mut time), "answered, the question goes");

    // Confirmed, they are discarded, and the question going asks nothing more.
    root.show_core(discarding(Some(DiscardReason::Close)));
    let _ = settle(&mut root, &mut time);
    let _ = press(&mut root, &mut time, Key::Tab);
    let commands = press(&mut root, &mut time, Key::Enter);
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ConfirmDiscard]),
        "Discard and close discards them: {commands:?}"
    );
    root.show_core(discarding(None));
    let commands = settle(&mut root, &mut time);
    assert!(commands.is_empty(), "{commands:?}");
    assert!(!shows_dialog(&mut root, &mut time));

    // Withdrawn unanswered, it goes answering nothing.
    root.show_core(discarding(Some(DiscardReason::Close)));
    let _ = settle(&mut root, &mut time);
    root.show_core(discarding(None));
    let commands = settle(&mut root, &mut time);
    assert!(commands.is_empty(), "a withdrawn question answers nothing: {commands:?}");
    assert!(!shows_dialog(&mut root, &mut time));

    // An account's sign-in ended: Continue in browser signs in again, which shows as adding
    // an account does, and closes once done, stopping nothing.
    root.show_core(reauthenticating(None));
    beui_paint_snapshot::assert_painting(
        "a_root_asks_to_sign_in_again_to_its_hosts_account",
        dialog_painting(&mut root, &mut time),
    );
    let commands = press(&mut root, &mut time, Key::Enter);
    assert!(
        matches!(commands.as_slice(), [CoreCommand::Reauthenticate]),
        "Continue in browser signs in again: {commands:?}"
    );
    root.show_core(reauthenticating(Some(SigningIn {
        browser: Some(BrowserSignIn {
            url: "https://example.com/auth".to_owned(),
            code: "WXYZ-1234".to_owned(),
        }),
    })));
    let commands = settle(&mut root, &mut time);
    assert!(commands.is_empty(), "the sign-in shows, stopping nothing: {commands:?}");
    assert!(shows_dialog(&mut root, &mut time));
    root.show_core(CoreView::default());
    let commands = settle(&mut root, &mut time);
    assert!(commands.is_empty(), "signed in again, nothing is stopped: {commands:?}");
    assert!(!shows_dialog(&mut root, &mut time), "signed in again, the dialog goes");

    // Closing the question leaves the account for the accounts, as be-host's own does.
    root.show_core(reauthenticating(None));
    let _ = settle(&mut root, &mut time);
    let commands = press(&mut root, &mut time, Key::Escape);
    assert!(
        matches!(commands.as_slice(), [CoreCommand::SwitchAccount]),
        "closed, it switches account: {commands:?}"
    );
    root.show_core(CoreView::default());
    let _ = settle(&mut root, &mut time);

    // F9 lists the invitations, which hold no link.
    let commands = press(&mut root, &mut time, OPENS_INVITATIONS);
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ListInvitations]),
        "opening them lists them: {commands:?}"
    );
    root.show_core(invitations());
    let listed = dialog_painting(&mut root, &mut time);
    beui_paint_snapshot::assert_painting("a_root_lists_its_hosts_invitations", listed.clone());

    // New link asks for one, whose link comes back once, as the request's answer.
    root.input(None, be_lib_ui_types::input::encode(&chord(Key::Enter, Modifiers::NONE)))
        .expect("the root takes input");
    let made = requested(&mut root, &mut time);
    assert!(matches!(made.1, SecretCommand::InviteMember), "{made:?}");
    // While one is asked, it asks for no other.
    let _ = press(&mut root, &mut time, Key::Enter);
    let _ = settle(&mut root, &mut time);
    assert!(root.take_secret_requests().is_empty(), "one request at a time");
    root.answer_secret(made.0, Some(CoreSecret::Link(Password::from(LINK.to_owned()))));
    let shown = dialog_painting(&mut root, &mut time);
    beui_paint_snapshot::assert_painting("a_root_shows_an_invitations_link", shown.clone());
    // Answered again, an id waits for nothing: the root shows only what it asked for.
    root.answer_secret(made.0, Some(CoreSecret::Link(Password::from("stray".to_owned()))));
    assert_eq!(dialog_painting(&mut root, &mut time), shown);

    // Copy puts it on the host's clipboard, as the person asked.
    let _ = press(&mut root, &mut time, Key::Enter);
    let copied = root.take_copies();
    assert_eq!(copied.len(), 1, "Copy copies the link once");

    // Done drops it, back in the list. A row's Link asks for its link again; one the host
    // could not make (answered with nothing) shows none, the list asking afresh.
    root.input(None, be_lib_ui_types::input::encode(&click(pos2(91.0, 168.0))))
        .expect("the root takes input");
    assert_eq!(dialog_painting(&mut root, &mut time), listed, "Done drops the link");
    let _ = root.take_core_commands();
    let _ = settle(&mut root, &mut time);
    root.input(None, be_lib_ui_types::input::encode(&click(pos2(222.0, 128.0))))
        .expect("the root takes input");
    let again = requested(&mut root, &mut time);
    assert!(matches!(again.1, SecretCommand::InvitationLink(MINE)), "{again:?}");
    root.answer_secret(again.0, None);
    assert_eq!(
        dialog_painting(&mut root, &mut time),
        listed,
        "a link not made shows none"
    );
    let commands = settle(&mut root, &mut time);
    assert!(commands.is_empty(), "{commands:?}");
    root.input(None, be_lib_ui_types::input::encode(&click(pos2(299.0, 128.0))))
        .expect("the root takes input");
    let commands = settle(&mut root, &mut time);
    assert!(
        matches!(commands.as_slice(), [CoreCommand::RevokeInvitation(MINE)]),
        "Revoke revokes it: {commands:?}"
    );

    // Closed, the link goes: opened again, the invitations show without it.
    let _ = press(&mut root, &mut time, Key::Escape);
    assert!(!shows_dialog(&mut root, &mut time));
    let commands = press(&mut root, &mut time, OPENS_INVITATIONS);
    assert!(matches!(commands.as_slice(), [CoreCommand::ListInvitations]));
    assert_eq!(
        dialog_painting(&mut root, &mut time),
        listed,
        "reopened, the invitations hold no link"
    );
}
