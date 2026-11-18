use be_lib_ui_types::{Key, Modifiers};
use be_sdk_root_testing::{DIALOG_LAYER, DIALOG_SURFACE, LOCK_LAYER, SCRIM_LAYER};
use be_wasm_root_api::{DisplayQuestion, LockState};

use super::*;

/// How long the person has to answer, in nanoseconds.
const WITHIN: u64 = 15_000_000_000;

/// The host's question of `round`, asked at the frame clock's `time`, or none.
fn asking(round: Option<u64>, time: u64) -> Option<DisplayQuestion> {
    round.map(|round| DisplayQuestion {
        round,
        revert_at: time + WITHIN,
    })
}

/// The session's lock, locked or not.
fn lock(locked: bool) -> LockState {
    LockState {
        generation: 0,
        available: true,
        locked,
        checking: false,
        refused: None,
        user: "Ada Lovelace".to_owned(),
        power: Vec::new(),
        retry_at: None,
    }
}

/// Draws frames from `*time` on until the root answers its host's display question, and
/// answers each round with whether it was kept.
fn answers(root: &mut Root, time: &mut u64) -> Vec<(u64, bool)> {
    (0..10)
        .find_map(|_| {
            *time += 1;
            step(root, *time);
            let answers = root.take_display_answers();
            (!answers.is_empty()).then_some(answers)
        })
        .unwrap_or_default()
}

#[test]
fn a_root_asks_to_keep_display_settings_its_host_changed() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);

    // The host asks: the question shows over its scrim, taking the focus.
    root.show_displays(asking(Some(7), time));
    let shown = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the question of keeping the display settings shows",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    assert!(layer_of(&shown, SCRIM_LAYER).is_some(), "over its scrim");
    for _ in 0..3 {
        time += 1;
        step(&mut root, time);
    }
    let painting = own_paintings(&mut root, time)
        .into_iter()
        .find(|(surface, _)| *surface == u64::from(DIALOG_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints the question");
    beui_paint_snapshot::assert_painting(
        "a_root_asks_to_keep_display_settings_its_host_changed",
        painting,
    );

    // Enter keeps them.
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(Key::Enter, Modifiers::NONE)),
    )
    .expect("the root takes input");
    assert_eq!(
        answers(&mut root, &mut time),
        vec![(7, true)],
        "Enter keeps the settings of the round asked"
    );
    root.show_displays(asking(None, time));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "answered, the question goes",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );

    // Escape reverts them.
    root.show_displays(asking(Some(8), time));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the next question shows",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    for _ in 0..2 {
        time += 1;
        step(&mut root, time);
    }
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(Key::Escape, Modifiers::NONE)),
    )
    .expect("the root takes input");
    assert_eq!(
        answers(&mut root, &mut time),
        vec![(8, false)],
        "Escape reverts them"
    );
    root.show_displays(asking(None, time));

    // Unanswered in time, it reverts them itself.
    root.show_displays(asking(Some(9), time));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the next question shows",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    time += WITHIN;
    assert_eq!(
        answers(&mut root, &mut time),
        vec![(9, false)],
        "once the time to answer ran out, it reverts them"
    );

    // The host withdrawing the question (it reverted on its own) closes it, answering nothing.
    root.show_displays(asking(Some(10), time));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the next question shows",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    root.show_displays(asking(None, time));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "withdrawn, the question goes",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );
    assert!(
        root.take_display_answers().is_empty(),
        "a question the host withdrew is answered nothing"
    );

    // A stale answer, to a round the host no longer asks, is dropped before the host sees it.
    root.show_displays(asking(Some(11), time));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the next question shows",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    root.show_displays(asking(Some(12), time));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "a new round replaces the question",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(Key::Enter, Modifiers::NONE)),
    )
    .expect("the root takes input");
    assert_eq!(
        answers(&mut root, &mut time),
        vec![(12, true)],
        "only the round asked is answered"
    );
    root.show_displays(asking(None, time));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "answered, the question goes",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );

    // A question asked while the session is locked shows once it is unlocked.
    root.show_lock(lock(true));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the lock screen shows",
        |done| layer_of(done, LOCK_LAYER).is_some(),
    );
    root.show_displays(asking(Some(13), time));
    for _ in 0..3 {
        time += 1;
        assert!(
            layer_of(&step(&mut root, time), DIALOG_LAYER).is_none(),
            "no question shows while locked"
        );
    }
    root.show_lock(lock(false));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "unlocked, the question asked meanwhile shows",
        |done| layer_of(done, LOCK_LAYER).is_none() && layer_of(done, DIALOG_LAYER).is_some(),
    );
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(Key::Enter, Modifiers::NONE)),
    )
    .expect("the root takes input");
    assert_eq!(answers(&mut root, &mut time), vec![(13, true)]);
    root.show_displays(asking(None, time));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "answered, the question goes",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );

    // A question replaces an open rename dialog, which closes without renaming; once the
    // question goes, the focus is back where it was before the rename opened, outside any
    // dialog, and the strip still shows its own text.
    let strip = text_painting(&mut root, time);
    open_dialog(&mut root, &mut time);
    root.input(
        None,
        be_lib_ui_types::input::encode(&[be_lib_ui_types::input::Input::Ime(
            be_lib_ui_types::input::ImeEvent::CommitText("Renamed".to_owned()),
        )
        .into()]),
    )
    .expect("the root takes input");
    root.show_displays(asking(Some(14), time));
    let asked = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the question replaces the rename dialog",
        |done| layer_of(done, DIALOG_LAYER).is_some() && done.ime.is_none(),
    );
    assert!(
        asked.ime.is_none(),
        "the rename dialog's field went with it"
    );
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(Key::Escape, Modifiers::NONE)),
    )
    .expect("the root takes input");
    assert_eq!(answers(&mut root, &mut time), vec![(14, false)]);
    root.show_displays(asking(None, time));
    let closed = until(
        &mut root,
        &mut time,
        Vec::new(),
        "answered, the question goes",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );
    assert!(
        closed.ime.is_none(),
        "no field has the focus: the rename dialog is gone"
    );
    assert_eq!(
        text_painting(&mut root, time),
        strip,
        "replaced, the rename dialog renamed nothing"
    );
    // F2 opens a rename dialog afresh: the root holds none closed underneath.
    open_dialog(&mut root, &mut time);
}
