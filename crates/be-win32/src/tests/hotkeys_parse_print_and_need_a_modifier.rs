use super::*;

#[test]
fn hotkeys_parse_print_and_need_a_modifier() {
    assert_eq!(Hotkey::parse("Ctrl+Alt+Space"), Ok(Hotkey::DEFAULT));
    assert_eq!(Hotkey::DEFAULT.to_string(), "Ctrl+Alt+Space");
    assert_eq!(
        Hotkey::DEFAULT.flags(),
        hotkey::MOD_CONTROL | hotkey::MOD_ALT | hotkey::MOD_NOREPEAT
    );
    let hotkey = Hotkey::parse("win + shift + f12").unwrap();
    assert_eq!(hotkey.key, 0x7B);
    assert_eq!(hotkey.to_string(), "Shift+Win+F12");
    assert_eq!(Hotkey::parse("Ctrl+q").unwrap().key, u32::from(b'Q'));
    assert_eq!(Hotkey::parse("Alt+0x7C").unwrap().to_string(), "Alt+F13");
    assert_eq!(Hotkey::parse("Space"), Err(HotkeyError::NoModifier));
    assert_eq!(Hotkey::parse("Ctrl+Alt"), Err(HotkeyError::NoKey));
    assert_eq!(
        Hotkey::parse("Ctrl+Banana"),
        Err(HotkeyError::UnknownKey("Banana".to_owned()))
    );
    assert_eq!(
        Hotkey::DEFAULT.registration_error(hotkey::ERROR_HOTKEY_ALREADY_REGISTERED, String::new()),
        HotkeyError::Taken(Hotkey::DEFAULT)
    );
    assert_eq!(
        Hotkey::DEFAULT.registration_error(5, "Access is denied.".to_owned()),
        HotkeyError::Failed("Access is denied.".to_owned())
    );
}
