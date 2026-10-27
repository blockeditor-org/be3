use beui_types::input::{Key, Modifiers};

pub fn prevents_default(
    key: Option<Key>,
    value: &str,
    modifiers: Modifiers,
    clipboard_keys: bool,
) -> bool {
    let Some(key) = key else {
        return false;
    };
    let clipboard = modifiers.ctrl && matches!(key, Key::C | Key::V | Key::X);
    if clipboard {
        return clipboard_keys && !(modifiers.shift && key == Key::V);
    }
    let printable = value.chars().count() == 1 && !modifiers.command();
    let passes = key.is_media() || key.is_modifier();
    !printable && !passes
}

#[cfg(target_arch = "wasm32")]
pub fn apple() -> bool {
    thread_local! {
        static APPLE: bool = web_sys::window()
            .and_then(|window| window.navigator().platform().ok())
            .is_some_and(|platform| platform.starts_with("Mac") || platform.starts_with("iP"));
    }
    APPLE.with(|apple| *apple)
}

#[cfg(target_arch = "wasm32")]
pub fn modifiers(alt: bool, ctrl: bool, meta: bool, shift: bool) -> Modifiers {
    Modifiers::with_meta(alt, ctrl, shift, meta, apple())
}

#[cfg(target_arch = "wasm32")]
pub fn key(event: &web_sys::KeyboardEvent) -> Option<Key> {
    let physical = Key::evdev_from_code(&event.code()).and_then(Key::from_evdev);
    Key::from_value(&event.key(), physical, apple())
}

#[cfg(target_arch = "wasm32")]
pub fn keystroke(event: &web_sys::KeyboardEvent, pressed: bool) -> beui_types::input::Keystroke {
    beui_types::input::Keystroke {
        key: key(event),
        pressed,
        repeat: pressed && event.repeat(),
        evdev: Key::evdev_from_code(&event.code()),
        text: None,
    }
}

#[cfg(test)]
mod tests;
