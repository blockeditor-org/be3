use std::cell::RefCell;
use std::rc::Rc;

use beui::reactive::{Memo, clone, create_effect, held_modifiers, on_global_key};
use beui::{GlobalKeyPress, Key, KeyChord, Modifiers};
use block_plugin_api::{EditorInstanceId, EditorRegion};

use crate::plugin_host;

#[derive(Default)]
struct Held {
    keys: Vec<Key>,
    modifiers: Modifiers,
}

pub(super) fn intercept_keys(
    plugin_id: String,
    instance: EditorInstanceId,
    region: EditorRegion,
    chords: Memo<Vec<KeyChord>>,
    allowed: impl Fn() -> bool + 'static,
) {
    let held = Rc::new(RefCell::new(Held::default()));
    on_global_key(clone!(plugin_id held -> move |global: GlobalKeyPress| {
        let press = global.press;
        let mut keys = held.borrow_mut();
        let forwarding = keys.keys.contains(&press.key);
        if !press.pressed {
            if !forwarding {
                return false;
            }
            keys.keys.retain(|key| *key != press.key);
            drop(keys);
            plugin_host::intercept_region(&plugin_id, instance, region, Some(press), press.modifiers);
            return true;
        }
        let asked = chords.with_untracked(|chords| {
            chords
                .iter()
                .any(|chord| chord.matches(press) && (chord.modifiers.command() || chord.key.is_media()))
        });
        if !(asked || forwarding && press.repeat) || !allowed() {
            return false;
        }
        if !forwarding {
            keys.keys.push(press.key);
        }
        keys.modifiers = press.modifiers;
        drop(keys);
        plugin_host::intercept_region(&plugin_id, instance, region, Some(press), press.modifiers);
        true
    }));
    let modifiers = held_modifiers();
    create_effect(move || {
        let now = modifiers.get();
        let mut keys = held.borrow_mut();
        if keys.modifiers == Modifiers::NONE {
            return;
        }
        keys.modifiers = Modifiers {
            alt: keys.modifiers.alt && now.alt,
            ctrl: keys.modifiers.ctrl && now.ctrl,
            shift: keys.modifiers.shift && now.shift,
            logo: keys.modifiers.logo && now.logo,
        };
        drop(keys);
        plugin_host::intercept_region(&plugin_id, instance, region, None, now);
    });
}
