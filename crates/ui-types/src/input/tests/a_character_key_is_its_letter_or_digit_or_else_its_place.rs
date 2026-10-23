use super::*;

#[test]
fn a_character_key_is_its_letter_or_digit_or_else_its_place() {
    for (layout, code, unshifted, shifted, key) in [
        ("US", "Digit1", "1", "!", Key::One),
        ("AZERTY", "Digit1", "&", "1", Key::One),
        ("German", "Digit7", "7", "/", Key::Seven),
        ("Czech", "Digit2", "ě", "2", Key::Two),
        ("AZERTY", "KeyQ", "a", "A", Key::A),
        ("US", "KeyQ", "q", "Q", Key::Q),
        ("Russian", "KeyC", "с", "С", Key::C),
        ("Polish AltGr", "KeyA", "ą", "Ą", Key::A),
        ("AZERTY", "KeyM", ",", "?", Key::M),
        ("US", "Semicolon", ";", ":", Key::Semicolon),
        ("US", "Equal", "=", "+", Key::Plus),
        ("US", "Backquote", "`", "~", Key::Backtick),
        ("US", "Space", " ", " ", Key::Space),
        ("US", "Numpad1", "1", "End", Key::One),
        ("US", "NumpadAdd", "+", "+", Key::Plus),
        ("dead key", "BracketLeft", "Dead", "Dead", Key::BracketLeft),
    ] {
        let physical = Key::evdev_from_code(code).and_then(Key::from_evdev);
        for value in [unshifted, shifted] {
            let expected = match value {
                "End" => Key::End,
                _ => key,
            };
            assert_eq!(
                Key::from_value(value, physical, false),
                Some(expected),
                "{layout} {code} typing {value:?}"
            );
        }
    }
    assert_eq!(Key::from_typed(None, Some(Key::Comma)), Some(Key::Comma));
    assert_eq!(Key::from_typed(Some('a'), None), Some(Key::A));
    assert_eq!(
        Key::from_value(" ", None, false),
        Some(Key::Space),
        "a space is Space even with no physical code"
    );
    assert_eq!(Key::from_value("ф", None, false), None);
    assert_eq!(Key::from_value("CapsLock", Some(Key::A), false), None);
    for (value, key) in [
        ("ArrowUp", Key::ArrowUp),
        ("Control", Key::Ctrl),
        ("F12", Key::F12),
        ("MediaPlay", Key::MediaPlayPause),
    ] {
        assert_eq!(Key::from_value(value, None, false), Some(key), "{value}");
    }
    assert_eq!(Key::from_value("Meta", None, true), Some(Key::Ctrl));
    assert_eq!(Key::from_value("Meta", None, false), Some(Key::Logo));
    for key in Key::ALL {
        if let Some(evdev) = key.evdev() {
            assert_eq!(Key::from_evdev(evdev), Some(key), "{key:?}");
        }
    }
}
