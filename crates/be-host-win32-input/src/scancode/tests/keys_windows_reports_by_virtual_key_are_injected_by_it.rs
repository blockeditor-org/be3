use super::*;

#[test]
fn keys_windows_reports_by_virtual_key_are_injected_by_it() {
    assert_eq!(evdev(VK_NUMLOCK, ScanCode::new(0x45, true)), Some(69));
    assert_eq!(evdev(VK_PAUSE, ScanCode::new(0x45, false)), Some(119));
    assert_eq!(
        evdev(0xAF, ScanCode::new(0x30, true)),
        Key::VolumeUp.evdev()
    );
    assert_eq!(
        evdev(0xB3, ScanCode::new(0, false)),
        Key::MediaPlayPause.evdev()
    );
    assert_eq!(
        evdev(0xA6, ScanCode::new(0x6A, true)),
        Key::BrowserBack.evdev()
    );
    for evdev in [69, 119, 113, 114, 115, 158, 163, 164, 165, 166] {
        assert_eq!(scan_of_evdev(evdev), None, "{evdev}");
        assert!(virtual_key_of_evdev(evdev).is_some(), "{evdev}");
    }
    assert_eq!(virtual_key_of_evdev(30), None);
}
