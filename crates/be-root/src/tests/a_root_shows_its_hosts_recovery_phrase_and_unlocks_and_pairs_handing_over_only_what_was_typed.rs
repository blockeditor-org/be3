use be_lib_geometry::pos2;
use be_lib_ui_types::input::{ImeEvent, Input};
use be_lib_ui_types::{Key, Modifiers};
use crate::{DIALOG_LAYER, DIALOG_SURFACE, ONBOARDING_LAYER, ONBOARDING_SURFACE};
use be_wasm_root_api::{
    CoreCommand, CoreFailure, CoreSecret, CoreView, CoreWait, PairingRequest, RecoveryArea,
    SecretCommand, UnlockArea,
};

use super::*;

const PHRASE: &str = "apple banana cherry date elder fig grape hazel iris juniper kale lime";

fn asks(
    root: &mut Root,
    time: &mut u64,
    events: Vec<be_lib_ui_types::input::Timed>,
) -> Vec<CoreCommand> {
    if !events.is_empty() {
        root.input(None, be_lib_ui_types::input::encode(&events))
            .expect("the root takes input");
    }
    for _ in 0..4 {
        *time += 1;
        step(root, *time);
    }
    root.take_core_commands()
}

fn key(key: Key) -> Vec<be_lib_ui_types::input::Timed> {
    chord(key, Modifiers::NONE)
}

fn typed(text: &str) -> Vec<be_lib_ui_types::input::Timed> {
    [ImeEvent::Enabled, ImeEvent::CommitText(text.to_owned())]
        .into_iter()
        .map(|event| Input::Ime(event).into())
        .collect()
}

fn painting(root: &mut Root, time: &mut u64, surface: u32) -> beui_paint_format::Painting {
    let _ = asks(root, time, Vec::new());
    own_paintings(root, *time)
        .into_iter()
        .find(|(id, _)| *id == u64::from(surface))
        .map(|(_, painting)| painting)
        .expect("the root paints the surface")
}

/// The one secret the root asks for in a few frames.
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

fn recovery() -> CoreView {
    CoreView {
        waits_for: Some(CoreWait::Recovery),
        recovery: RecoveryArea {
            words: 12,
            checked: vec![2, 5],
            replacing: false,
            busy: false,
            error: None,
        },
        ..CoreView::default()
    }
}

fn unlock() -> CoreView {
    CoreView {
        waits_for: Some(CoreWait::Unlock),
        unlock: UnlockArea {
            account: "Ada".to_owned(),
            pairing: false,
            error: None,
        },
        ..CoreView::default()
    }
}

fn failed() -> CoreView {
    CoreView {
        waits_for: Some(CoreWait::Failure),
        failure: Some(CoreFailure {
            message: "This device's copy of the blocks cannot be read.".to_owned(),
            unsaved: 3,
        }),
        ..CoreView::default()
    }
}

#[test]
fn a_root_shows_its_hosts_recovery_phrase_and_unlocks_and_pairs_handing_over_only_what_was_typed() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);

    // Waiting for the phrase to be written down, the root asks for it, once, and shows it.
    root.show_core(recovery());
    let (id, asked) = requested(&mut root, &mut time);
    assert!(matches!(asked, SecretCommand::RecoveryPhrase), "{asked:?}");
    root.answer_secret(
        id,
        Some(CoreSecret::Phrase(Password::from(PHRASE.to_owned()))),
    );
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_recovery_phrase",
        painting(&mut root, &mut time, ONBOARDING_SURFACE),
    );

    // The words at the checked positions cross as typed, inside the command alone.
    let _ = asks(&mut root, &mut time, click(pos2(200.0, 197.0)));
    let _ = asks(&mut root, &mut time, typed("banana elder"));
    let commands = asks(&mut root, &mut time, key(Key::Enter));
    match commands.as_slice() {
        [CoreCommand::ConfirmRecovery(words)] => assert_eq!(words.as_bytes(), b"banana elder"),
        other => panic!("the words confirm the phrase: {other:?}"),
    }

    // Unlocking with the phrase, or pairing: the code comes back once, and shows.
    root.show_core(unlock());
    beui_paint_snapshot::assert_painting(
        "a_root_unlocks_its_hosts_keys",
        painting(&mut root, &mut time, ONBOARDING_SURFACE),
    );
    let _ = asks(&mut root, &mut time, click(pos2(155.0, 113.0)));
    let _ = asks(&mut root, &mut time, typed(PHRASE));
    let commands = asks(&mut root, &mut time, click(pos2(344.0, 113.0)));
    match commands.as_slice() {
        [CoreCommand::UnlockWithPhrase(phrase)] => assert_eq!(phrase.as_bytes(), PHRASE.as_bytes()),
        other => panic!("Unlock hands over the phrase: {other:?}"),
    }
    root.input(
        None,
        be_lib_ui_types::input::encode(&click(pos2(200.0, 158.0))),
    )
    .expect("the root takes input");
    let (id, asked) = requested(&mut root, &mut time);
    assert!(matches!(asked, SecretCommand::PairingCode), "{asked:?}");
    root.answer_secret(
        id,
        Some(CoreSecret::Code(Password::from("482-913".to_owned()))),
    );
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_pairing_code",
        painting(&mut root, &mut time, ONBOARDING_SURFACE),
    );

    // After a failure: Restart, Quit, or deleting this device's copy, asked first.
    root.show_core(failed());
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_failure",
        painting(&mut root, &mut time, ONBOARDING_SURFACE),
    );
    let commands = asks(&mut root, &mut time, click(pos2(55.0, 102.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::Restart]),
        "{commands:?}"
    );
    let commands = asks(&mut root, &mut time, click(pos2(286.0, 102.0)));
    assert!(
        matches!(commands.as_slice(), [CoreCommand::Exit]),
        "{commands:?}"
    );
    let commands = asks(&mut root, &mut time, click(pos2(176.0, 102.0)));
    assert!(commands.is_empty(), "asking deletes nothing: {commands:?}");
    beui_paint_snapshot::assert_painting(
        "a_root_asks_before_deleting_its_hosts_local_copy",
        painting(&mut root, &mut time, ONBOARDING_SURFACE),
    );
    root.show_core(CoreView {
        pairing: vec![PairingRequest {
            id: 7,
            device: "Laptop".to_owned(),
            refused: None,
        }],
        ..CoreView::default()
    });
    beui_paint_snapshot::assert_painting(
        "a_root_asks_to_pair_a_device",
        painting(&mut root, &mut time, DIALOG_SURFACE),
    );
    // The code that device shows crosses as typed; Not now puts the request aside for good.
    let _ = asks(&mut root, &mut time, click(pos2(100.0, 137.0)));
    let _ = asks(&mut root, &mut time, typed("482913"));
    let commands = asks(&mut root, &mut time, key(Key::Enter));
    match commands.as_slice() {
        [CoreCommand::ApprovePairing(approval)] => {
            assert_eq!(approval.id, 7);
            assert_eq!(approval.code.as_bytes(), b"482913");
        }
        other => panic!("Approve approves it with the code typed: {other:?}"),
    }
    let commands = asks(&mut root, &mut time, key(Key::Escape));
    assert!(
        commands.is_empty(),
        "Not now asks nothing of the host: {commands:?}"
    );
    time += 1;
    assert!(
        layer_of(&step(&mut root, time), DIALOG_LAYER).is_none(),
        "put aside, the request is asked about no more, though still listed"
    );

    // Once nothing waits, the onboarding goes.
    time += 1;
    assert!(layer_of(&step(&mut root, time), ONBOARDING_LAYER).is_none());
}
