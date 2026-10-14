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
