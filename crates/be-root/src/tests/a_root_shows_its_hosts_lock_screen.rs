use be_lib_ui_types::input::{ImeEvent, Input};
use be_lib_ui_types::{Key, Modifiers};
use be_wasm_root_api::{LockState, PowerAction};

use super::*;
use crate::{LOCK_LAYER, LOCK_SURFACE};

fn locked(refused: Option<&str>, retry_at: Option<u64>) -> LockState {
    LockState {
        generation: 0,
        available: true,
        locked: true,
        checking: false,
        refused: refused.map(str::to_owned),
        user: "Ada Lovelace".to_owned(),
        power: vec![PowerAction::Suspend, PowerAction::PowerOff],
        retry_at,
    }
}

#[test]
fn a_root_shows_its_hosts_lock_screen() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    show_core(&mut root, be_wasm_root_api::CoreView::default());

    // The host locks the session, which shows the lock screen over everything, its field
    // taking the input method.
    root.show_lock(locked(None, None));
    let locked_frame = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the lock screen shows, its password field taking the input method",
        |done| layer_of(done, LOCK_LAYER).is_some() && done.ime.is_some(),
    );
    assert_eq!(
        shown(&locked_frame).last(),
        Some(&LOCK_LAYER),
        "over everything"
    );

    // What is typed is handed to the host as typed.
    let events: Vec<be_lib_ui_types::input::Timed> = [
        ImeEvent::Enabled,
        ImeEvent::CommitText("hunter2".to_owned()),
    ]
    .into_iter()
    .map(|event| Input::Ime(event).into())
    .chain(chord(Key::Enter, Modifiers::NONE))
    .collect();
    hand(&mut root, &mut time, events);
    let typed = (0..6).find_map(|_| {
        time += 1;
        step(&mut root, time);
        root.take_unlock()
    });
    assert_eq!(
        typed.as_ref().and_then(Password::as_str),
        Some("hunter2"),
        "the password is handed to the host as typed"
    );

    // The host refuses it and makes the person wait, which the lock screen counts down.
    root.show_lock(locked(
        Some("That password is not right."),
        Some(time + 5_000_000_000),
    ));
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_lock_screen",
        painting_of(&mut root, &mut time, LOCK_SURFACE),
    );
}
