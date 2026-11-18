use be_sdk_root_testing::{SLOW_SAVE, TOAST_LAYER, TOAST_SURFACE};
use be_wasm_root_api::{CoreView, Saving, SyncState};

use super::*;

fn core(unsaved: bool, saves: u64) -> CoreView {
    CoreView {
        generation: 0,
        saving: Saving {
            unsaved,
            saves,
            unsent: 0,
            sync: SyncState::Syncing,
        },
    }
}

#[test]
fn a_root_shows_the_slow_saves_toast_while_its_hosts_core_has_changes_unsaved() {
    let mut root = embedding();
    step(&mut root, 1);
    root.show_core(core(true, 1));
    let slow = u64::try_from(SLOW_SAVE.as_nanos()).expect("the wait fits");
    for time in 2..8 {
        assert!(
            layer_of(&step(&mut root, time), TOAST_LAYER).is_none(),
            "while saving is not yet slow, no toast shows"
        );
    }
    let mut time = slow + 1;
    let shown = (0..8).any(|_| {
        time += 1;
        layer_of(&step(&mut root, time), TOAST_LAYER).is_some()
    });
    assert!(shown, "once changes stayed unsaved that long, the slow save's toast shows");
    let painting = own_paintings(&mut root, time)
        .into_iter()
        .find(|(surface, _)| *surface == u64::from(TOAST_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its toasts");
    beui_paint_snapshot::assert_painting(
        "a_root_shows_the_slow_saves_toast_while_its_hosts_core_has_changes_unsaved",
        painting,
    );

    // Saved, it goes.
    root.show_core(core(false, 2));
    let gone = until(
        &mut root,
        &mut time,
        Vec::new(),
        "once the changes are saved, the toast goes",
        |done| layer_of(done, TOAST_LAYER).is_none(),
    );
    assert!(layer_of(&gone, TOAST_LAYER).is_none());
}
