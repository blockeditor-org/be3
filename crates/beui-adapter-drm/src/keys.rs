use beui::Key;
use smithay::input::keyboard::{keysyms, xkb};

pub fn key(keysym: u32, evdev: u32) -> Option<Key> {
    let character = char::from_u32(xkb::keysym_to_utf32(xkb::Keysym::new(keysym)))
        .filter(|character| *character != '\0');
    named(keysym).or_else(|| Key::from_typed(character, Key::from_evdev(evdev)))
}

fn named(keysym: u32) -> Option<Key> {
    let key = match keysym {
        keysyms::KEY_Down => Key::ArrowDown,
        keysyms::KEY_Left => Key::ArrowLeft,
        keysyms::KEY_Right => Key::ArrowRight,
        keysyms::KEY_Up => Key::ArrowUp,
        keysyms::KEY_BackSpace => Key::Backspace,
        keysyms::KEY_Delete | keysyms::KEY_KP_Delete => Key::Delete,
        keysyms::KEY_End | keysyms::KEY_KP_End => Key::End,
        keysyms::KEY_Return | keysyms::KEY_KP_Enter => Key::Enter,
        keysyms::KEY_Escape => Key::Escape,
        keysyms::KEY_Home | keysyms::KEY_KP_Home => Key::Home,
        keysyms::KEY_Next | keysyms::KEY_KP_Next => Key::PageDown,
        keysyms::KEY_Prior | keysyms::KEY_KP_Prior => Key::PageUp,
        keysyms::KEY_Tab | keysyms::KEY_ISO_Left_Tab => Key::Tab,
        keysyms::KEY_Insert | keysyms::KEY_KP_Insert => Key::Insert,
        keysyms::KEY_XF86Back => Key::BrowserBack,
        keysyms::KEY_XF86AudioRaiseVolume => Key::VolumeUp,
        keysyms::KEY_XF86AudioLowerVolume => Key::VolumeDown,
        keysyms::KEY_XF86AudioMute => Key::VolumeMute,
        keysyms::KEY_XF86AudioMicMute => Key::MicMute,
        keysyms::KEY_XF86MonBrightnessUp => Key::BrightnessUp,
        keysyms::KEY_XF86MonBrightnessDown => Key::BrightnessDown,
        keysyms::KEY_XF86AudioPlay | keysyms::KEY_XF86AudioPause => Key::MediaPlayPause,
        keysyms::KEY_XF86AudioNext => Key::MediaNext,
        keysyms::KEY_XF86AudioPrev => Key::MediaPrevious,
        keysyms::KEY_XF86AudioStop => Key::MediaStop,
        keysyms::KEY_Shift_L | keysyms::KEY_Shift_R => Key::Shift,
        keysyms::KEY_Control_L | keysyms::KEY_Control_R => Key::Ctrl,
        keysyms::KEY_Alt_L | keysyms::KEY_Alt_R | keysyms::KEY_Meta_L | keysyms::KEY_Meta_R => {
            Key::Alt
        }
        keysyms::KEY_Super_L | keysyms::KEY_Super_R => Key::Logo,
        keysyms::KEY_F1..=keysyms::KEY_F24 => {
            Key::function((keysym - keysyms::KEY_F1 + 1) as usize)?
        }
        _ => return None,
    };
    Some(key)
}

pub fn virtual_terminal(keysym: u32) -> Option<i32> {
    (keysyms::KEY_XF86Switch_VT_1..=keysyms::KEY_XF86Switch_VT_12)
        .contains(&keysym)
        .then(|| (keysym - keysyms::KEY_XF86Switch_VT_1 + 1) as i32)
}

#[cfg(test)]
mod tests;
