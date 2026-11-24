use crate::{LOCK_LAYER, SLOW_SAVE, TOAST_LAYER, TOAST_SURFACE};
use be_lib_geometry::pos2;
use be_wasm_root_api::{LockState, SavingState, SyncState};

use super::*;

fn saving(unsaved: bool, saves: u64) -> SavingState {
    SavingState {
        generation: 0,
        unsaved,
        saves,
        unsent: 0,
        sync: SyncState::Syncing,
    }
}

/// Whether the toasts' layer shows in a frame drawn at `time`.
fn toast_at(root: &mut Root, time: u64) -> bool {
    layer_of(&step(root, time), TOAST_LAYER).is_some()
}

#[test]
fn a_root_shows_the_slow_saves_toast_while_its_hosts_changes_stay_unsaved() {
    let mut root = embedding();
    let second = 1_000_000_000;
    let slow = u64::try_from(SLOW_SAVE.as_nanos()).expect("the wait fits");
    step(&mut root, 1);
    root.show_saving(saving(true, 1));
    for time in 2..6 {
        assert!(
            !toast_at(&mut root, time),
            "while saving is not yet slow, no toast shows"
        );
    }

    // A save finishing just before the wait is up starts it afresh.
    // (The root hears it with the frame after its clock moved there: what reaches a document
    // between frames counts from the last frame's time.)
    let restarted = slow - second;
    step(&mut root, restarted);
    root.show_saving(saving(true, 2));
    for time in restarted + 1..restarted + 3 {
        assert!(!toast_at(&mut root, time));
    }
    assert!(
        !toast_at(&mut root, slow + second),
        "a save finishing restarted the wait, so none shows once the first would have been up"
    );
    let mut time = restarted + slow + second;
    let shown = (0..8).any(|_| {
        time += 1;
        toast_at(&mut root, time)
    });
    assert!(
        shown,
        "once changes stayed unsaved that long since the last save, it shows"
    );
    // It slides in from the side and comes to rest within a second.
    time += second;
    step(&mut root, time);
    let painting = own_paintings(&mut root, time)
        .into_iter()
        .find(|(surface, _)| *surface == u64::from(TOAST_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its toasts");
    beui_paint_snapshot::assert_painting(
        "a_root_shows_the_slow_saves_toast_while_its_hosts_changes_stay_unsaved",
        painting,
    );

    // Dismissed, it stays dismissed while the changes are still unsaved.
    let close = pos2(window().width - 40.0, 36.0);
    // Pressed, it slides off, and goes once it is off.
    until(&mut root, &mut time, click(close), "a frame is drawn", |_| true);
    time += second;
    step(&mut root, time);
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "its close button dismisses it",
        |done| layer_of(done, TOAST_LAYER).is_none(),
    );
    for _ in 0..3 {
        time += slow;
        assert!(
            !toast_at(&mut root, time),
            "dismissed, it stays dismissed while still unsaved"
        );
    }

    // Saved, then slow again, it shows again; but not over the lock screen.
    root.show_saving(saving(false, 3));
    time += 1;
    step(&mut root, time);
    root.show_saving(saving(true, 3));
    time += 1;
    step(&mut root, time);
    time += slow + second;
    let again = (0..8).any(|_| {
        time += 1;
        toast_at(&mut root, time)
    });
    assert!(again, "saved and slow again, it shows again");
    root.show_lock(LockState {
        generation: 0,
        available: true,
        locked: true,
        checking: false,
        refused: None,
        user: "Ada Lovelace".to_owned(),
        power: Vec::new(),
        retry_at: None,
    });
    let locked = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the session locks",
        |done| layer_of(done, LOCK_LAYER).is_some(),
    );
    assert!(
        layer_of(&locked, TOAST_LAYER).is_none(),
        "no toast shows over the lock screen"
    );
}
