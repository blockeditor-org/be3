use be_lib_ui_types::{Key, Modifiers};
use be_sdk_root_testing::{DIALOG_LAYER, DIALOG_SURFACE, SCRIM_LAYER};
use be_wasm_root_api::{CoreCommand, CoreView, KeepDisplay, Saving, SyncState};

use super::*;

/// How long the person has to answer, in nanoseconds.
const WITHIN: u64 = 15_000_000_000;

fn asking(round: Option<u64>) -> CoreView {
    CoreView {
        generation: 0,
        saving: Saving {
            unsaved: false,
            saves: 0,
            unsent: 0,
            sync: SyncState::Saved,
        },
        keep_display: round.map(|round| KeepDisplay {
            round,
            answer_within: WITHIN,
        }),
    }
}

/// Draws frames from `*time` on until the root asks its host's core something, and answers
/// what it asked.
fn commands(root: &mut Root, time: &mut u64) -> Vec<CoreCommand> {
    (0..10)
        .find_map(|_| {
            *time += 1;
            step(root, *time);
            let commands = root.take_core_commands();
            (!commands.is_empty()).then_some(commands)
        })
        .unwrap_or_default()
}

#[test]
fn a_root_asks_to_keep_display_settings_and_answers_its_hosts_core() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);

    // The core asks: the question shows over its scrim, taking the focus.
    root.show_core(asking(Some(7)));
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
        "a_root_asks_to_keep_display_settings_and_answers_its_hosts_core",
        painting,
    );

    // Enter keeps them.
    root.input(
        None,
        be_lib_ui_types::input::encode(&chord(Key::Enter, Modifiers::NONE)),
    )
    .expect("the root takes input");
    assert_eq!(
        commands(&mut root, &mut time),
        vec![CoreCommand::KeepDisplay(7)],
        "Enter keeps the settings of the round asked"
    );
    root.show_core(asking(None));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "answered, the question goes",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );

    // Escape reverts them.
    root.show_core(asking(Some(8)));
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
        commands(&mut root, &mut time),
        vec![CoreCommand::RevertDisplay(8)],
        "Escape reverts them"
    );
    root.show_core(asking(None));

    // Unanswered in time, it reverts them itself.
    root.show_core(asking(Some(9)));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the next question shows",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    time += WITHIN;
    assert_eq!(
        commands(&mut root, &mut time),
        vec![CoreCommand::RevertDisplay(9)],
        "once the time to answer ran out, it reverts them"
    );

    // The core withdrawing the question (it reverted on its own) closes it, asking nothing.
    root.show_core(asking(Some(10)));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "the next question shows",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    root.show_core(asking(None));
    until(
        &mut root,
        &mut time,
        Vec::new(),
        "withdrawn, the question goes",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );
    assert!(
        root.take_core_commands().is_empty(),
        "a question the core withdrew is answered nothing"
    );
}
