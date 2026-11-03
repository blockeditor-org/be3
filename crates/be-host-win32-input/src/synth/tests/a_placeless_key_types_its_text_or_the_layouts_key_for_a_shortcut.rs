use super::*;

fn german(wanted: char) -> Option<LayoutKey> {
    match wanted {
        'y' => Some(LayoutKey { vk: 0x59, scan: 0x2C }),
        'z' => Some(LayoutKey { vk: 0x5A, scan: 0x15 }),
        '-' => Some(LayoutKey { vk: 0xBD, scan: 0x35 }),
        _ => None,
    }
}

fn placeless(key: Key, text: Option<&str>, pressed: bool) -> Keystroke {
    Keystroke {
        key: Some(key),
        pressed,
        repeat: false,
        evdev: None,
        text: text.map(str::to_owned),
    }
}

#[test]
fn a_placeless_key_types_its_text_or_the_layouts_key_for_a_shortcut() {
    assert_eq!(
        keystroke(&placeless(Key::Z, Some("z"), true), &german),
        text("z")
    );
    assert_eq!(keystroke(&placeless(Key::Z, Some("z"), false), &german), []);
    assert_eq!(
        keystroke(&placeless(Key::Y, None, true), &german),
        [Synth::Key {
            vk: 0x59,
            scan: 0x2C,
            flags: 0
        }]
    );
    assert_eq!(
        keystroke(&placeless(Key::Z, None, false), &german),
        [Synth::Key {
            vk: 0x5A,
            scan: 0x15,
            flags: KEY_UP
        }]
    );
    assert_eq!(
        keystroke(&placeless(Key::Minus, None, true), &german),
        [Synth::Key {
            vk: 0xBD,
            scan: 0x35,
            flags: 0
        }]
    );
    assert_eq!(
        keystroke(&placeless(Key::C, None, true), &|_| None),
        [Synth::Key {
            vk: u16::from(b'C'),
            scan: 0,
            flags: 0
        }]
    );
    assert_eq!(
        keystroke(&placeless(Key::Enter, Some("\r"), true), &german),
        [key(Key::Enter.evdev().unwrap(), true).unwrap()]
    );
    assert_eq!(
        keystroke(&placeless(Key::ArrowLeft, None, false), &german),
        [key(Key::ArrowLeft.evdev().unwrap(), false).unwrap()]
    );
    assert_eq!(
        keystroke(&placeless(Key::Backtick, None, true), &german),
        [key(Key::Backtick.evdev().unwrap(), true).unwrap()]
    );
}
