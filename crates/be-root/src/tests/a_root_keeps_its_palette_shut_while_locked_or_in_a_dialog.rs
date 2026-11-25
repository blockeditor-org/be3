use be_lib_ui_types::{Key, Modifiers};
use be_wasm_root_api::{CoreView, LockState};

use super::*;
use crate::{DIALOG_LAYER, LOCK_LAYER, PALETTE_LAYER};

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

fn palette_chord() -> Vec<be_lib_ui_types::input::Timed> {
    chord(
        Key::P,
        Modifiers {
            shift: true,
            ..Modifiers::CTRL
        },
    )
}

/// Whether the palette shows, a few frames on.
fn palette_shows(
    root: &mut Root,
    time: &mut u64,
    events: Vec<be_lib_ui_types::input::Timed>,
) -> bool {
    hand(root, time, events);
    *time += 1;
    layer_of(&step(root, *time), PALETTE_LAYER).is_some()
}

#[test]
fn a_root_keeps_its_palette_shut_while_locked_or_in_a_dialog() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    show_core(&mut root, CoreView::default());

    // Ctrl+Shift+P opens the palette and Escape closes it; P alone opens nothing.
    assert!(!palette_shows(
        &mut root,
        &mut time,
        chord(Key::P, Modifiers::NONE)
    ));
    assert!(palette_shows(&mut root, &mut time, palette_chord()));
    assert!(!palette_shows(
        &mut root,
        &mut time,
        chord(Key::Escape, Modifiers::NONE)
    ));

    // The lock coming up shuts it and takes the keyboard; while locked the chord opens nothing.
    assert!(palette_shows(&mut root, &mut time, palette_chord()));
    root.show_lock(lock(true));
    let locked = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the lock screen shows and the palette is shut",
        |done| layer_of(done, LOCK_LAYER).is_some() && layer_of(done, PALETTE_LAYER).is_none(),
    );
    assert_eq!(shown(&locked).last(), Some(&LOCK_LAYER), "over everything");
    assert!(!palette_shows(&mut root, &mut time, palette_chord()));
    root.show_lock(lock(false));
    let _ = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the lock screen goes",
        |done| layer_of(done, LOCK_LAYER).is_none(),
    );

    // With the root's dialog up the chord opens nothing either, and Escape closes the dialog.
    let _ = menu(&mut root, &mut time, Entry::Contacts);
    let _ = until(
        &mut root,
        &mut time,
        Vec::new(),
        "the contacts show",
        |done| layer_of(done, DIALOG_LAYER).is_some(),
    );
    assert!(!palette_shows(&mut root, &mut time, palette_chord()));
    let _ = until(
        &mut root,
        &mut time,
        chord(Key::Escape, Modifiers::NONE),
        "Escape closes the contacts",
        |done| layer_of(done, DIALOG_LAYER).is_none(),
    );
    assert!(palette_shows(&mut root, &mut time, palette_chord()));
}
