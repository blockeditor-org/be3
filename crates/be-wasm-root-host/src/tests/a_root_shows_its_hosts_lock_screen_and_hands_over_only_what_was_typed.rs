use be_lib_ui_types::input::{ImeEvent, Input};
use be_lib_ui_types::{Key, Modifiers};
use be_sdk_root_testing::{DIALOG_LAYER, LOCK_LAYER, LOCK_SURFACE, LOCKS_SESSION};
use be_wasm_root_api::{LockState, PowerAction};

use super::*;

fn locked(refused: Option<&str>) -> LockState {
    LockState {
        generation: 0,
        available: true,
        locked: true,
        checking: false,
        refused: refused.map(str::to_owned),
        user: "Ada Lovelace".to_owned(),
        power: vec![PowerAction::Suspend, PowerAction::PowerOff],
    }
}

/// Types `text` through the input method and presses Enter.
fn typed(text: &str) -> Vec<be_lib_ui_types::input::Timed> {
    let mut events: Vec<be_lib_ui_types::input::Timed> = [
        ImeEvent::Enabled,
        ImeEvent::SetComposingText(String::new()),
        ImeEvent::CommitText(text.to_owned()),
    ]
    .into_iter()
    .map(|event| Input::Ime(event).into())
    .collect();
    events.extend(chord(Key::Enter, Modifiers::NONE));
    events
}

#[test]
fn a_root_shows_its_hosts_lock_screen_and_hands_over_only_what_was_typed() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);

    // F4 asks the host to lock the session, which the host does by showing the root it locked.
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(LOCKS_SESSION, Modifiers::NONE)),
    )
    .expect("the root takes input");
    let asked = (0..6).any(|_| {
        time += 1;
        step(&mut root, time);
        root.take_lock_asked()
    });
    assert!(asked, "F4 asks the host to lock the session");
    root.show_lock(locked(None));
    let shown = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the lock screen shows, its password field taking the input method",
        |done| layer_of(done, LOCK_LAYER).is_some() && done.ime.is_some(),
    );
    assert_eq!(
        super::shown(&shown).last(),
        Some(&LOCK_LAYER),
        "the lock screen is over everything"
    );

    // While locked, the root takes no key of its own: F2 opens no dialog.
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(Key::F2, Modifiers::NONE)),
    )
    .expect("the root takes input");
    for _ in 0..3 {
        time += 1;
        assert!(layer_of(&step(&mut root, time), DIALOG_LAYER).is_none());
    }

    // What is typed is handed to the host to check, as typed, and the host's no is all the
    // root hears back, with why through the lock.
    root.input(None, be_lib_ui_types::input::encode(&typed("hunter2")))
        .expect("the root takes input");
    let unlocks = (0..8)
        .find_map(|_| {
            time += 1;
            step(&mut root, time);
            let unlocks = root.take_unlocks();
            (!unlocks.is_empty()).then_some(unlocks)
        })
        .expect("Enter in the field hands the password to the host");
    let [(id, password)] = &unlocks[..] else {
        panic!("one password is handed over: {unlocks:?}");
    };
    assert_eq!(password.as_bytes(), b"hunter2");
    root.answer_unlock(*id, false);
    root.show_lock(locked(Some("That password is not right.")));
    for _ in 0..4 {
        time += 1;
        step(&mut root, time);
    }
    let painting = own_paintings(&mut root, time)
        .into_iter()
        .find(|(surface, _)| *surface == u64::from(LOCK_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its lock screen");
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_lock_screen_and_hands_over_only_what_was_typed",
        painting,
    );

    // The right one unlocks it: the lock screen goes.
    root.input(None, be_lib_ui_types::input::encode(&typed("correct")))
        .expect("the root takes input");
    let unlocks = (0..8)
        .find_map(|_| {
            time += 1;
            step(&mut root, time);
            let unlocks = root.take_unlocks();
            (!unlocks.is_empty()).then_some(unlocks)
        })
        .expect("the field, cleared, takes a second password");
    assert_eq!(
        unlocks
            .iter()
            .map(|(_, password)| password.as_bytes().to_vec())
            .collect::<Vec<_>>(),
        vec![b"correct".to_vec()],
        "the field was cleared as the first was handed over"
    );
    root.answer_unlock(unlocks[0].0, true);
    root.show_lock(LockState {
        locked: false,
        ..locked(None)
    });
    let unlocked = until(
        &mut root,
        &mut time,
        Vec::new(),
        "once unlocked, the lock screen goes",
        |done| layer_of(done, LOCK_LAYER).is_none(),
    );
    assert!(unlocked.ime.is_none(), "nothing edits text once unlocked");
}
