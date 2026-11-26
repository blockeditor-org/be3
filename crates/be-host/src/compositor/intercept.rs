use be_sdk_api::{EditorInstanceId, ViewId};
use beui::KeyPhase;
use beui::native::try_with_document;
use beui::reactive::*;
use beui::{KeyChord, KeyEvent, KeyInput, Modifiers};

pub(super) fn intercept_keys(
    app_id: Stored<String>,
    instance: EditorInstanceId,
    region: ViewId,
    chords: Memo<Vec<KeyChord>>,
    allowed: impl Fn() -> bool + 'static,
) {
    let held = StoredCell::new(Modifiers::NONE);
    let allowed = move || {
        allowed()
            && !be_host_runtime::view_has_keyboard(&app_id.value(), instance, region)
            && !host_modal_open()
    };
    on_key(
        KeyAt::Window,
        KeyPhase::Capture,
        KeyReveal::Focus,
        move |event: KeyEvent| {
            let press = match event.input {
                KeyInput::Tap(key) => {
                    let asked = chords.with_untracked(|chords| {
                        chords.iter().any(|chord| chord.tap && chord.key == key)
                    });
                    if asked && allowed() {
                        be_host_runtime::intercept_tap(&app_id.value(), instance, region, key);
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
                held.set(press.modifiers);
            }
            be_host_runtime::intercept_view(
                &app_id.value(),
                instance,
                region,
                Some(press),
                press.modifiers,
            );
            true
        },
    );
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
        be_host_runtime::intercept_view(&app_id.value(), instance, region, None, now);
    });
}

/// Whether one of be-host's own layers holds the keyboard (its command palette, a prompt): the
/// shell's intercepts are not offered past it, as a document's own actions stop at its dialogs.
/// The lock screen is the exception: it limits keys itself, and lets media keys through.
fn host_modal_open() -> bool {
    try_with_document(|document| {
        let layers = document.layers();
        layers.takes_keyboard() && !layers.locked()
    })
    .unwrap_or(false)
}
