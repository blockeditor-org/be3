use be_lib_ui_types::{Key, Modifiers};
use be_sdk_root_testing::{DIALOG_LAYER, DIALOG_SURFACE, SCRIM_LAYER};
use be_wasm_root_api::DisplayQuestion;

use super::*;

/// How long the person has to answer, in nanoseconds.
const WITHIN: u64 = 15_000_000_000;

fn asking(round: Option<u64>) -> Option<DisplayQuestion> {
    round.map(|round| DisplayQuestion {
        round,
        answer_within: WITHIN,
    })
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
    root.show_displays(asking(Some(7)));
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
    root.show_displays(asking(None));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "answered, the question goes",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );

    // Escape reverts them.
    root.show_displays(asking(Some(8)));
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
    root.show_displays(asking(None));

    // Unanswered in time, it reverts them itself.
    root.show_displays(asking(Some(9)));
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
    root.show_displays(asking(Some(10)));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the next question shows",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    root.show_displays(asking(None));
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
}
