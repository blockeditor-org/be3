use be_lib_ui_types::input::{ImeEvent, Input};
use be_lib_ui_types::{Key, Modifiers};
use be_wasm_root_api::{LockState, MemoryPoint, PowerAction, StatisticRow};

use super::*;
use crate::{LOCK_LAYER, LOCK_SURFACE, STATISTICS_LAYER, STATISTICS_SURFACE};

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

fn row(name: &str, current: &str, average: &str, peak: &str) -> StatisticRow {
    StatisticRow {
        heading: false,
        name: name.to_owned(),
        current: current.to_owned(),
        average: average.to_owned(),
        peak: peak.to_owned(),
    }
}

fn heading(name: &str) -> StatisticRow {
    StatisticRow {
        heading: true,
        ..row(name, "", "", "")
    }
}

#[test]
fn a_root_shows_its_hosts_lock_screen_and_statistics() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    root.show_core(be_wasm_root_api::CoreView::default());

    // Performance opens the statistics panel, which watches the host's statistics and graphs
    // its memory over the last minutes: a gap of more than two minutes, while the computer
    // slept, breaks the lines.
    let _ = menu(&mut root, &mut time, Entry::Performance);
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the statistics panel opens",
        |done| layer_of(done, STATISTICS_LAYER).is_some(),
    );
    assert!(root.watching_statistics());
    root.show_statistics(
        vec![
            row("Build", "release", "", ""),
            row("Full frame", "4.0 ms", "4.2 ms", "9.1 ms"),
            heading("Frame pacing"),
            row("Waiting for apps", "0.3 ms", "0.4 ms", "1.2 ms"),
        ],
        vec![
            heading("Memory"),
            row("Resident", "182 MB", "180 MB", "190 MB"),
        ],
        None,
    );
    const MIB: u64 = 1 << 20;
    let minute = |at: u64, resident: u64| MemoryPoint {
        at,
        resident: resident * MIB,
        anonymous: resident * MIB * 2 / 3,
        shared: 12 * MIB,
        wasm: resident * MIB / 3,
    };
    let start = 1_763_000_000;
    root.show_memory_history(
        vec![
            minute(start, 150),
            minute(start + 60, 165),
            minute(start + 120, 172),
            minute(start + 600, 176),
            minute(start + 660, 182),
        ],
        start + 660,
    );
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_statistics_and_memory",
        painting_of(&mut root, &mut time, STATISTICS_SURFACE),
    );
    let _ = menu(&mut root, &mut time, Entry::Performance);
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the statistics panel closes",
        |done| layer_of(done, STATISTICS_LAYER).is_none(),
    );

    // Lock asks the host to lock the session, which shows the lock screen over everything,
    // its field taking the input method.
    let _ = menu(&mut root, &mut time, Entry::Lock);
    assert!(root.take_lock_asked(), "Lock asks the host to lock the session");
    root.show_lock(locked(None, None));
    let locked_frame = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the lock screen shows, its password field taking the input method",
        |done| layer_of(done, LOCK_LAYER).is_some() && done.ime.is_some(),
    );
    assert_eq!(shown(&locked_frame).last(), Some(&LOCK_LAYER), "over everything");

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
