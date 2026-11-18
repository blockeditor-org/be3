use be_lib_geometry::{Rect, pos2, vec2};
use be_lib_ui_types::input::{ImeEvent, Input};
use be_lib_ui_types::{ImeArea, Key, Modifiers};
use be_sdk_root_testing::{
    DIALOG_ORIGIN, DIALOG_SIZE, LOCK_LAYER, LOCK_SURFACE, LOCKS_SESSION, ORIGIN, STATISTICS_LAYER,
    TOAST_LAYER, UNLOCKS_TWICE,
};
use be_wasm_root_api::{LockState, Notice, PowerAction};

use super::*;

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

/// The input method's area a frame asks for.
fn ime(done: &FrameDone) -> Option<ImeArea> {
    done.ime
        .as_deref()
        .map(|area| ImeArea::decode(area).expect("the input method's area reads back"))
}

/// Hands the root `events`, then draws a few frames, and answers the password it handed its
/// host over them, if it did.
fn handed(
    root: &mut Root,
    time: &mut u64,
    events: Vec<be_lib_ui_types::input::Timed>,
) -> Option<String> {
    root.input(None, be_lib_ui_types::input::encode(&events))
        .expect("the root takes input");
    (0..8).find_map(|_| {
        *time += 1;
        step(root, *time);
        root.take_unlock()
            .map(|password| password.as_str().expect("typed as text").to_owned())
    })
}

#[test]
fn a_root_shows_its_hosts_lock_screen_and_hands_over_only_what_was_typed() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    let stray = || chord(UNLOCKS_TWICE, Modifiers::NONE);

    // A password handed over while the session is not locked reaches no one.
    assert_eq!(
        handed(&mut root, &mut time, stray()),
        None,
        "while the session is not locked, a password handed over is dropped unchecked"
    );

    // The dialog's field has the focus, and a notice shows, when the session locks.
    let dialog_open = open_dialog(&mut root, &mut time);
    let dialog = Rect::from_min_size(
        pos2(DIALOG_ORIGIN.0, DIALOG_ORIGIN.1),
        vec2(DIALOG_SIZE.0 as f32, DIALOG_SIZE.1 as f32),
    );
    let in_dialog =
        |done: &FrameDone| ime(done).is_some_and(|area| dialog.contains_rect(area.rect));
    assert!(in_dialog(&dialog_open));
    root.show_notices(vec![Notice {
        id: 1,
        message: "Only the owner can rename this folder.".to_owned(),
        failure: true,
        actions: Vec::new(),
    }]);
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the notice shows",
        |done| layer_of(done, TOAST_LAYER).is_some(),
    );

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
    root.show_lock(locked(None, None));
    let shown = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the lock screen shows, its password field taking the input method",
        |done| {
            layer_of(done, LOCK_LAYER).is_some()
                && ime(done).is_some_and(|area| !dialog.contains_rect(area.rect))
        },
    );
    assert_eq!(
        super::shown(&shown).last(),
        Some(&LOCK_LAYER),
        "the lock screen is over everything"
    );
    assert!(
        layer_of(&shown, TOAST_LAYER).is_none(),
        "no toast shows over the lock screen"
    );
    let field = ime(&shown).expect("the lock screen's field edits text");
    assert!(
        field.rect.width() > DIALOG_SIZE.0 as f32,
        "the input method is on the lock screen's field, wider than the dialog's: {:?}",
        field.rect
    );
    let lock = layer_of(&shown, LOCK_LAYER).expect("the lock screen shows");
    assert!(
        lock.input.captures && lock.input.focuses,
        "the lock screen takes every press and the focus"
    );
    let layers = LayerList::decode(&shown.layers).expect("the frame is a layer list");
    let over_app = pos2(ORIGIN.0 + 10.0, ORIGIN.1 + 10.0);
    assert_eq!(
        layers.windows[0]
            .layers
            .iter()
            .rev()
            .find(|layer| layer.shown && layer.takes(over_app))
            .map(|layer| layer.id),
        Some(LOCK_LAYER),
        "a press over the app lands on the lock screen"
    );

    // While locked, the root takes none of its own keys: F3 opens no panel.
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(Key::F3, Modifiers::NONE)),
    )
    .expect("the root takes input");
    for _ in 0..3 {
        time += 1;
        assert!(layer_of(&step(&mut root, time), STATISTICS_LAYER).is_none());
    }

    // What is typed is handed to the host, once, as typed; while it is being checked, another
    // is dropped unchecked.
    assert_eq!(
        handed(&mut root, &mut time, typed("hunter2")).as_deref(),
        Some("hunter2")
    );
    assert_eq!(
        handed(&mut root, &mut time, stray()),
        None,
        "while one is being checked, another is dropped"
    );

    // The host refuses it and makes the person wait, which the lock screen counts down, and
    // drops one handed over meanwhile.
    root.show_lock(locked(
        Some("That password is not right."),
        Some(time + 5_000_000_000),
    ));
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
    assert_eq!(
        handed(&mut root, &mut time, stray()),
        None,
        "while the host makes the person wait, a password is dropped"
    );

    // The wait over, the right one unlocks it: the lock screen goes, the toast shows again and
    // the focus is back in the dialog's field.
    root.show_lock(locked(Some("That password is not right."), None));
    assert_eq!(
        handed(&mut root, &mut time, typed("correct")).as_deref(),
        Some("correct"),
        "the field was cleared as the first was handed over"
    );
    root.show_lock(LockState {
        locked: false,
        ..locked(None, None)
    });
    let unlocked = until(
        &mut root,
        &mut time,
        Vec::new(),
        "once unlocked, the lock screen goes and the dialog has the focus back",
        |done| layer_of(done, LOCK_LAYER).is_none() && in_dialog(done),
    );
    assert!(
        layer_of(&unlocked, TOAST_LAYER).is_some(),
        "the notice shows again"
    );
}
