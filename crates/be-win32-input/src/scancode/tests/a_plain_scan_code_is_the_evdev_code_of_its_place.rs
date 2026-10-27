use super::*;

#[test]
fn a_plain_scan_code_is_the_evdev_code_of_its_place() {
    for (scan, key) in [
        (0x1E, Key::A),
        (0x10, Key::Q),
        (0x02, Key::One),
        (0x0B, Key::Zero),
        (0x01, Key::Escape),
        (0x1C, Key::Enter),
        (0x2A, Key::Shift),
        (0x1D, Key::Ctrl),
        (0x38, Key::Alt),
        (0x39, Key::Space),
        (0x3B, Key::F1),
        (0x57, Key::F11),
        (0x58, Key::F12),
        (0x64, Key::F13),
        (0x76, Key::F24),
    ] {
        let code = ScanCode::new(scan, false);
        assert_eq!(evdev_of_scan(code), key.evdev(), "{scan:#x}");
        assert_eq!(scan_of_evdev(key.evdev().unwrap()), Some(code), "{key:?}");
    }
}
