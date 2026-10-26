use beui_core::input::{Key, Modifiers};

pub(crate) fn prevents_default(
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

#[cfg(test)]
mod tests;
