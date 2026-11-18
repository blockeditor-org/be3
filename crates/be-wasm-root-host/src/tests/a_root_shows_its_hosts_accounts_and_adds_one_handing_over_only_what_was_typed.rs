use be_lib_geometry::pos2;
use be_lib_ui_types::input::{ImeEvent, Input};
use be_lib_ui_types::{Key, Modifiers};
use be_sdk_root_testing::{DIALOG_LAYER, DIALOG_SURFACE, ONBOARDING_LAYER, ONBOARDING_SURFACE};
use be_wasm_root_api::{
    AccountRow, BrowserSignIn, CoreCommand, CoreView, CoreWait, DisplayQuestion, NewAccount,
    SigningIn,
};

use super::*;

/// The block stack's view while it waits for an account, with `accounts`.
fn choosing(accounts: Vec<AccountRow>) -> CoreView {
    CoreView {
        waits_for: Some(CoreWait::Account),
        accounts,
        ..CoreView::default()
    }
}

/// An account kept on this device alone.
fn ada() -> AccountRow {
    AccountRow {
        key: "local\u{0}ada".to_owned(),
        name: "Ada".to_owned(),
        email: String::new(),
        server: None,
        has_last_organization: false,
        current: false,
        organizations: Vec::new(),
    }
}

/// An account on a server, in an organization.
fn grace() -> AccountRow {
    AccountRow {
        key: "https://example.com\u{0}grace".to_owned(),
        name: "Grace".to_owned(),
        email: "grace@example.com".to_owned(),
        server: Some("https://example.com".to_owned()),
        has_last_organization: true,
        current: false,
        organizations: vec!["Analytical Engines".to_owned()],
    }
}

/// Hands the root `events` and draws a few frames, answering what it asked of the block stack.
fn asks(
    root: &mut Root,
    time: &mut u64,
    events: Vec<be_lib_ui_types::input::Timed>,
) -> Vec<CoreCommand> {
    root.input(None, be_lib_ui_types::input::encode(&events))
        .expect("the root takes input");
    for _ in 0..4 {
        *time += 1;
        step(root, *time);
    }
    root.take_core_commands()
}

/// `text` committed through the input method into the focused field.
fn typed(text: &str) -> Vec<be_lib_ui_types::input::Timed> {
    [ImeEvent::Enabled, ImeEvent::CommitText(text.to_owned())]
        .into_iter()
        .map(|event| Input::Ime(event).into())
        .collect()
}

/// What the root paints `surface` with, a few frames from `*time` on.
fn painting_of(root: &mut Root, time: &mut u64, surface: u32) -> beui_paint_format::Painting {
    for _ in 0..3 {
        *time += 1;
        step(root, *time);
    }
    own_paintings(root, *time)
        .into_iter()
        .find(|(id, _)| *id == u64::from(surface))
        .map(|(_, painting)| painting)
        .expect("the root paints the surface")
}

#[test]
fn a_root_shows_its_hosts_accounts_and_adds_one_handing_over_only_what_was_typed() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    root.show_core(choosing(vec![ada(), grace()]));
    let shown = until(
        &mut root,
        &mut time,
        Vec::new(),
        "while the block stack waits for an account, the accounts show",
        |done| layer_of(done, ONBOARDING_LAYER).is_some(),
    );
    let layer = layer_of(&shown, ONBOARDING_LAYER).expect("the accounts show");
    assert!(
        layer.input.captures && layer.input.focuses,
        "the accounts take every press and the focus"
    );
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_accounts",
        painting_of(&mut root, &mut time, ONBOARDING_SURFACE),
    );

    // Open, and the chevron beside it, open the account, back where it was or choosing.
    let commands = asks(&mut root, &mut time, click(pos2(297.0, 175.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::OpenAccount(key)] if *key == ada().key),
        "Open opens the account: {commands:?}"
    );
    let commands = asks(&mut root, &mut time, click(pos2(351.0, 175.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ChooseOrganization(key)] if *key == ada().key),
        "the chevron opens it choosing its organization: {commands:?}"
    );

    // Removing one kept on this device alone asks first.
    let commands = asks(&mut root, &mut time, click(pos2(351.0, 117.0)));
    assert!(commands.is_empty(), "{commands:?}");
    let commands = asks(&mut root, &mut time, click(pos2(277.0, 151.0)));
    assert!(commands.is_empty(), "asking removes nothing: {commands:?}");
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "removing an account kept on this device alone asks first",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    beui_paint_snapshot::assert_painting(
        "a_root_asks_before_removing_an_account_kept_on_this_device",
        painting_of(&mut root, &mut time, DIALOG_SURFACE),
    );
    let _ = asks(&mut root, &mut time, chord(Key::Tab, Modifiers::NONE));
    let commands = asks(&mut root, &mut time, chord(Key::Enter, Modifiers::NONE));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::ForgetAccount(key)] if *key == ada().key),
        "confirmed, it is forgotten: {commands:?}"
    );

    // With no accounts, Add account opens the dialog, which starts afresh.
    root.show_core(choosing(Vec::new()));
    let _ = asks(&mut root, &mut time, Vec::new());
    let _ = asks(&mut root, &mut time, chord(Key::Tab, Modifiers::NONE));
    let commands = asks(&mut root, &mut time, chord(Key::Enter, Modifiers::NONE));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::CancelSignIn]),
        "opening the dialog drops what was being added: {commands:?}"
    );
    beui_paint_snapshot::assert_painting(
        "a_root_adds_an_account",
        painting_of(&mut root, &mut time, DIALOG_SURFACE),
    );

    // To a server: its address, and the gateway token, which crosses inside the command alone.
    let commands = asks(&mut root, &mut time, click(pos2(140.0, 122.0)));
    assert!(commands.is_empty(), "{commands:?}");
    let _ = asks(&mut root, &mut time, typed("https://example.com"));
    let _ = asks(&mut root, &mut time, chord(Key::Tab, Modifiers::NONE));
    let _ = asks(&mut root, &mut time, typed("s3cret"));
    let commands = asks(&mut root, &mut time, chord(Key::Enter, Modifiers::NONE));
    match commands.as_slice() {
        [CoreCommand::AddAccount(NewAccount::Server(server))] => {
            assert_eq!(server.url, "https://example.com");
            assert_eq!(
                server.gateway.as_ref().map(|gateway| gateway.as_bytes()),
                Some(&b"s3cret"[..]),
                "the gateway token is handed over as typed"
            );
        }
        other => panic!("Continue adds the account on the server: {other:?}"),
    }

    // While it connects, then while the person signs in in their browser.
    root.show_core(CoreView {
        signing_in: Some(SigningIn { browser: None }),
        ..choosing(Vec::new())
    });
    let _ = asks(&mut root, &mut time, Vec::new());
    root.show_core(CoreView {
        signing_in: Some(SigningIn {
            browser: Some(BrowserSignIn {
                url: "https://example.com/auth".to_owned(),
                code: "WXYZ-1234".to_owned(),
            }),
        }),
        ..choosing(Vec::new())
    });
    let signing_in = painting_of(&mut root, &mut time, DIALOG_SURFACE);
    beui_paint_snapshot::assert_painting(
        "a_root_waits_for_its_hosts_browser_sign_in",
        signing_in.clone(),
    );
    let commands = asks(&mut root, &mut time, chord(Key::Enter, Modifiers::NONE));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::OpenSignInPage]),
        "Open browser opens the sign-in's page: {commands:?}"
    );

    // The host's question of keeping display settings replaces it without stopping the sign-in,
    // which shows again once the question goes.
    root.show_displays(Some(DisplayQuestion {
        round: 1,
        revert_at: time + 15_000_000_000,
    }));
    let commands = asks(&mut root, &mut time, Vec::new());
    assert!(
        commands.is_empty(),
        "the question stops no sign-in: {commands:?}"
    );
    assert_ne!(
        painting_of(&mut root, &mut time, DIALOG_SURFACE),
        signing_in,
        "the question shows in the dialog's place"
    );
    let _ = asks(&mut root, &mut time, chord(Key::Enter, Modifiers::NONE));
    assert_eq!(root.take_display_answers(), vec![(1, true)]);
    root.show_displays(None);
    let commands = asks(&mut root, &mut time, Vec::new());
    assert!(
        commands.is_empty(),
        "resumed, it starts nothing afresh: {commands:?}"
    );
    assert_eq!(
        painting_of(&mut root, &mut time, DIALOG_SURFACE),
        signing_in,
        "once the question goes, the sign-in shows again"
    );

    // Closing it stops adding.
    let commands = asks(&mut root, &mut time, chord(Key::Escape, Modifiers::NONE));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::CancelSignIn]),
        "closing the dialog stops the sign-in: {commands:?}"
    );
    root.show_core(choosing(Vec::new()));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "closed, the dialog goes",
        |done| layer_of(done, DIALOG_LAYER).is_none() && layer_of(done, ONBOARDING_LAYER).is_some(),
    );

    // On this device, under a name; once added, the dialog closes by itself, stopping nothing.
    let commands = asks(&mut root, &mut time, chord(Key::Enter, Modifiers::NONE));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::CancelSignIn]),
        "Add account opens the dialog again, the focus back on it: {commands:?}"
    );
    let _ = asks(&mut root, &mut time, click(pos2(294.0, 122.0)));
    let _ = asks(&mut root, &mut time, typed("Lin"));
    let commands = asks(&mut root, &mut time, chord(Key::Enter, Modifiers::NONE));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::AddAccount(NewAccount::Local(name))] if name == "Lin"),
        "Create account adds one on this device: {commands:?}"
    );
    let lin = AccountRow {
        key: "local\u{0}lin".to_owned(),
        name: "Lin".to_owned(),
        ..ada()
    };
    root.show_core(choosing(vec![lin]));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "added, the dialog closes",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );
    assert!(
        root.take_core_commands().is_empty(),
        "the dialog closed for the account added stops nothing"
    );

    // Once the block stack has an account open, the accounts go.
    root.show_core(CoreView {
        waits_for: None,
        ..choosing(Vec::new())
    });
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "an account open, the accounts go",
        |done| layer_of(done, ONBOARDING_LAYER).is_none(),
    );
}
