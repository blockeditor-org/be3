use beui::Key;
use smithay::input::keyboard::keysyms;

pub fn key(keysym: u32) -> Option<Key> {
    let key = match keysym {
        keysyms::KEY_Down => Key::ArrowDown,
        keysyms::KEY_Left => Key::ArrowLeft,
        keysyms::KEY_Right => Key::ArrowRight,
        keysyms::KEY_Up => Key::ArrowUp,
        keysyms::KEY_BackSpace => Key::Backspace,
        keysyms::KEY_bracketleft | keysyms::KEY_braceleft => Key::BracketLeft,
        keysyms::KEY_bracketright | keysyms::KEY_braceright => Key::BracketRight,
        keysyms::KEY_Delete | keysyms::KEY_KP_Delete => Key::Delete,
        keysyms::KEY_End | keysyms::KEY_KP_End => Key::End,
        keysyms::KEY_Return | keysyms::KEY_KP_Enter => Key::Enter,
        keysyms::KEY_Escape => Key::Escape,
        keysyms::KEY_Home | keysyms::KEY_KP_Home => Key::Home,
        keysyms::KEY_minus | keysyms::KEY_underscore | keysyms::KEY_KP_Subtract => Key::Minus,
        keysyms::KEY_Next | keysyms::KEY_KP_Next => Key::PageDown,
        keysyms::KEY_Prior | keysyms::KEY_KP_Prior => Key::PageUp,
        keysyms::KEY_equal | keysyms::KEY_plus | keysyms::KEY_KP_Add => Key::Plus,
        keysyms::KEY_space => Key::Space,
        keysyms::KEY_Tab | keysyms::KEY_ISO_Left_Tab => Key::Tab,
        keysyms::KEY_0 | keysyms::KEY_parenright | keysyms::KEY_KP_0 => Key::Zero,
        keysyms::KEY_1 | keysyms::KEY_exclam | keysyms::KEY_KP_1 => Key::One,
        keysyms::KEY_2 | keysyms::KEY_at | keysyms::KEY_KP_2 => Key::Two,
        keysyms::KEY_3 | keysyms::KEY_numbersign | keysyms::KEY_KP_3 => Key::Three,
        keysyms::KEY_4 | keysyms::KEY_dollar | keysyms::KEY_KP_4 => Key::Four,
        keysyms::KEY_5 | keysyms::KEY_percent | keysyms::KEY_KP_5 => Key::Five,
        keysyms::KEY_6 | keysyms::KEY_asciicircum | keysyms::KEY_KP_6 => Key::Six,
        keysyms::KEY_7 | keysyms::KEY_ampersand | keysyms::KEY_KP_7 => Key::Seven,
        keysyms::KEY_8 | keysyms::KEY_asterisk | keysyms::KEY_KP_8 => Key::Eight,
        keysyms::KEY_9 | keysyms::KEY_parenleft | keysyms::KEY_KP_9 => Key::Nine,
        keysyms::KEY_grave | keysyms::KEY_asciitilde => Key::Backtick,
        keysyms::KEY_Insert | keysyms::KEY_KP_Insert => Key::Insert,
        keysyms::KEY_comma | keysyms::KEY_less => Key::Comma,
        keysyms::KEY_period | keysyms::KEY_greater | keysyms::KEY_KP_Decimal => Key::Period,
        keysyms::KEY_slash | keysyms::KEY_question | keysyms::KEY_KP_Divide => Key::Slash,
        keysyms::KEY_backslash | keysyms::KEY_bar => Key::Backslash,
        keysyms::KEY_semicolon | keysyms::KEY_colon => Key::Semicolon,
        keysyms::KEY_apostrophe | keysyms::KEY_quotedbl => Key::Quote,
        keysyms::KEY_XF86Back => Key::BrowserBack,
        keysyms::KEY_F1..=keysyms::KEY_F24 => FUNCTION[(keysym - keysyms::KEY_F1) as usize],
        _ => letter(keysym)?,
    };
    Some(key)
}

const FUNCTION: [Key; 24] = [
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
    Key::F10,
    Key::F11,
    Key::F12,
    Key::F13,
    Key::F14,
    Key::F15,
    Key::F16,
    Key::F17,
    Key::F18,
    Key::F19,
    Key::F20,
    Key::F21,
    Key::F22,
    Key::F23,
    Key::F24,
];

const LETTERS: [Key; 26] = [
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::G,
    Key::H,
    Key::I,
    Key::J,
    Key::K,
    Key::L,
    Key::M,
    Key::N,
    Key::O,
    Key::P,
    Key::Q,
    Key::R,
    Key::S,
    Key::T,
    Key::U,
    Key::V,
    Key::W,
    Key::X,
    Key::Y,
    Key::Z,
];

fn letter(keysym: u32) -> Option<Key> {
    let index = match keysym {
        keysyms::KEY_a..=keysyms::KEY_z => keysym - keysyms::KEY_a,
        keysyms::KEY_A..=keysyms::KEY_Z => keysym - keysyms::KEY_A,
        _ => return None,
    };
    Some(LETTERS[index as usize])
}

pub fn virtual_terminal(keysym: u32) -> Option<i32> {
    (keysyms::KEY_XF86Switch_VT_1..=keysyms::KEY_XF86Switch_VT_12)
        .contains(&keysym)
        .then(|| (keysym - keysyms::KEY_XF86Switch_VT_1 + 1) as i32)
}

#[cfg(test)]
mod tests;
