use super::*;

#[test]
fn web_codes_and_typed_characters_name_the_same_keys() {
    for (code, character, key) in [
        ("KeyQ", 'Q', Key::Q),
        ("Digit1", '!', Key::One),
        ("Equal", '+', Key::Plus),
        ("Slash", '?', Key::Slash),
        ("Backquote", '~', Key::Backtick),
    ] {
        assert_eq!(Key::from_code(code), Some(key));
        assert_eq!(Key::from_char(character), Some(key));
    }
    assert_eq!(Key::from_code("F24"), Some(Key::F24));
    assert_eq!(Key::from_code("F25"), None);
    assert_eq!(Key::from_code("KeyAB"), None);
    assert_eq!(Key::from_code("NumpadEnter"), Some(Key::Enter));
    for (value, key) in [
        ("q", Key::Q),
        ("Q", Key::Q),
        ("ArrowUp", Key::ArrowUp),
        ("Control", Key::Ctrl),
        ("F12", Key::F12),
        ("MediaPlay", Key::MediaPlayPause),
    ] {
        assert_eq!(Key::from_logical(value, false), Some(key), "{value}");
    }
    assert_eq!(Key::from_logical("Meta", true), Some(Key::Ctrl));
    assert_eq!(Key::from_logical("Meta", false), Some(Key::Logo));
    assert_eq!(
        Key::from_logical("ф", false),
        None,
        "a character with no key of its own leaves the adapter to name the key by its place"
    );
    assert_eq!(Key::from_logical("CapsLock", false), None);
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
