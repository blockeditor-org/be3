use super::*;

#[test]
fn web_codes_and_typed_characters_name_the_same_keys() {
    let place = |code: &str| Key::evdev_from_code(code).and_then(Key::from_evdev);
    for (code, character, key) in [
        ("KeyQ", "Q", Key::Q),
        ("Digit1", "!", Key::One),
        ("Equal", "+", Key::Plus),
        ("Slash", "?", Key::Slash),
        ("Backquote", "~", Key::Backtick),
    ] {
        assert_eq!(place(code), Some(key), "{code}");
        assert_eq!(Key::from_value(character, place(code), false), Some(key));
    }
    assert_eq!(place("F24"), Some(Key::F24));
    assert_eq!(place("F25"), None);
    assert_eq!(place("KeyAB"), None);
    assert_eq!(place("NumpadEnter"), Some(Key::Enter));
    assert_eq!(Key::meta(true), Key::Ctrl);
    assert_eq!(
        Modifiers::with_meta(false, false, false, true, true),
        Modifiers::CTRL,
        "Command is Ctrl on Apple's platforms"
    );
    assert_eq!(
        Modifiers::with_meta(false, false, false, true, false),
        Modifiers::LOGO
    );
}
