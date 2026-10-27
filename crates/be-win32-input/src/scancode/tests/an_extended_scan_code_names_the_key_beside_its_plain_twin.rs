use super::*;

#[test]
fn an_extended_scan_code_names_the_key_beside_its_plain_twin() {
    for (scan, key) in [
        (0x48, Key::ArrowUp),
        (0x50, Key::ArrowDown),
        (0x4B, Key::ArrowLeft),
        (0x4D, Key::ArrowRight),
        (0x47, Key::Home),
        (0x4F, Key::End),
        (0x49, Key::PageUp),
        (0x51, Key::PageDown),
        (0x52, Key::Insert),
        (0x53, Key::Delete),
        (0x5B, Key::Logo),
    ] {
        let code = ScanCode::new(scan, true);
        assert_eq!(evdev_of_scan(code), key.evdev(), "{scan:#x}");
        assert_eq!(scan_of_evdev(key.evdev().unwrap()), Some(code), "{key:?}");
    }
    assert_eq!(evdev_of_scan(ScanCode::new(0x48, false)), Some(72));
    assert_eq!(evdev_of_scan(ScanCode::new(0x1D, true)), Some(97));
    assert_eq!(evdev_of_scan(ScanCode::new(0x38, true)), Some(100));
    assert_eq!(evdev_of_scan(ScanCode::new(0x1C, true)), Some(96));
}
