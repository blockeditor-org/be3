use std::cell::Cell;

use beui::reactive::*;
use beui::{KeyChord, KeyEvent, KeyInput, Modifiers};
use block_plugin_api::{EditorInstanceId, EditorRegion};


pub(super) fn intercept_keys(
    plugin_id: Stored<String>,
    instance: EditorInstanceId,
    region: EditorRegion,
    chords: Memo<Vec<KeyChord>>,
    allowed: impl Fn() -> bool + 'static,
) {
    let held = store(Cell::new(Modifiers::NONE));
    on_global_key(move |event: KeyEvent| {
        let press = match event.input {
            KeyInput::Tap(key) => {
                let asked = chords.with_untracked(|chords| {
                    chords.iter().any(|chord| chord.tap && chord.key == key)
                });
                if asked && allowed() {
                    block_app_plugin_host::intercept_tap(&plugin_id.value(), instance, region, key);
                }
                return false;
            }
            KeyInput::Press(press) => press,
        };
        if !event.held() {
            let asked = chords.with_untracked(|chords| {
                chords.iter().any(|chord| {
                    chord.matches(press) && (chord.modifiers.command() || chord.key.is_media())
                })
            });
            if !press.pressed || !asked || !allowed() {
                return false;
            }
            held.with(|held| held.set(press.modifiers));
        }
        block_app_plugin_host::intercept_region(
            &plugin_id.value(),
            instance,
            region,
            Some(press),
            press.modifiers,
        );
        true
    });
    let modifiers = held_modifiers();
    create_effect(move || {
        let now = modifiers.get();
        let was = held.with(Cell::get);
        if was == Modifiers::NONE {
            return;
        }
        held.with(|held| {
            held.set(Modifiers {
                alt: was.alt && now.alt,
                ctrl: was.ctrl && now.ctrl,
                shift: was.shift && now.shift,
                logo: was.logo && now.logo,
            })
        });
        block_app_plugin_host::intercept_region(&plugin_id.value(), instance, region, None, now);
    });
}
