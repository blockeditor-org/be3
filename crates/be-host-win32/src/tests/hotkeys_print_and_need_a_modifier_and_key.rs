use super::*;

#[test]
fn hotkeys_print_and_need_a_modifier_and_key() {
    assert_eq!(Hotkey::DEFAULT.to_string(), "Ctrl+Alt+Space");
    assert_eq!(
        Hotkey::DEFAULT.flags(),
        hotkey::MOD_CONTROL | hotkey::MOD_ALT | hotkey::MOD_NOREPEAT
    );
    let hotkey = Hotkey {
        modifiers: Modifiers {
            shift: true,
            win: true,
            ..Modifiers::default()
        },
        key: 0x7B,
    };
    assert_eq!(hotkey.to_string(), "Shift+Win+F12");
    let no_modifier = Hotkey {
        modifiers: Modifiers::default(),
        key: 0x20,
    };
    assert_eq!(no_modifier.check(), Err(HotkeyError::NoModifier));
    let no_key = Hotkey {
        key: 0,
        ..Hotkey::DEFAULT
    };
    assert_eq!(no_key.check(), Err(HotkeyError::NoKey));
    assert_eq!(
        Hotkey::DEFAULT.registration_error(hotkey::ERROR_HOTKEY_ALREADY_REGISTERED, String::new()),
        HotkeyError::Taken(Hotkey::DEFAULT)
    );
    assert_eq!(
        Hotkey::DEFAULT.registration_error(5, "Access is denied.".to_owned()),
        HotkeyError::Failed("Access is denied.".to_owned())
    );
}
