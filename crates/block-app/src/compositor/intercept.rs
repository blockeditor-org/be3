use std::cell::Cell;
use std::rc::Rc;

use beui::reactive::{Memo, clone, create_effect, held_modifiers, on_global_key};
use beui::{GlobalKeyPress, KeyChord, Modifiers};
use block_plugin_api::{EditorInstanceId, EditorRegion};

use crate::plugin_host;

pub(super) fn intercept_keys(
    plugin_id: String,
    instance: EditorInstanceId,
    region: EditorRegion,
    chords: Memo<Vec<KeyChord>>,
    allowed: impl Fn() -> bool + 'static,
) {
    let held = Rc::new(Cell::new(Modifiers::NONE));
    on_global_key(clone!(plugin_id held -> move |global: GlobalKeyPress| {
        let press = global.press;
        if !global.held {
            let asked = chords.with_untracked(|chords| {
                chords
                    .iter()
                    .any(|chord| chord.matches(press) && (chord.modifiers.command() || chord.key.is_media()))
            });
            if !press.pressed || global.tap || !asked || !allowed() {
                return false;
            }
            held.set(press.modifiers);
        }
        plugin_host::intercept_region(&plugin_id, instance, region, Some(press), press.modifiers);
        true
    }));
    let modifiers = held_modifiers();
    create_effect(move || {
        let now = modifiers.get();
        let was = held.get();
        if was == Modifiers::NONE {
            return;
        }
        held.set(Modifiers {
            alt: was.alt && now.alt,
            ctrl: was.ctrl && now.ctrl,
            shift: was.shift && now.shift,
            logo: was.logo && now.logo,
        });
        plugin_host::intercept_region(&plugin_id, instance, region, None, now);
    });
}
