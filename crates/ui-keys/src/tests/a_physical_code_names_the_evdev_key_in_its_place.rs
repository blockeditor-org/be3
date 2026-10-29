use super::*;

#[test]
fn a_physical_code_names_the_evdev_key_in_its_place() {
    for (code, evdev) in [
        ("KeyA", 30),
        ("KeyQ", 16),
        ("Digit1", 2),
        ("Digit0", 11),
        ("ShiftLeft", 42),
        ("ShiftRight", 54),
        ("ControlRight", 97),
        ("MetaLeft", 125),
        ("Enter", 28),
        ("NumpadEnter", 96),
        ("Numpad1", 79),
        ("F12", 88),
        ("F13", 183),
        ("IntlBackslash", 86),
        ("BrowserBack", 158),
    ] {
        assert_eq!(Key::evdev_from_code(code), Some(evdev), "{code}");
    }
    assert_eq!(Key::evdev_from_code("Unidentified"), None);
}
