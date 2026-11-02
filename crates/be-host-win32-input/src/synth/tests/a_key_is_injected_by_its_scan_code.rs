use super::*;

#[test]
fn a_key_is_injected_by_its_scan_code() {
    assert_eq!(
        key(Key::A.evdev().unwrap(), true),
        Some(Synth::Key {
            vk: 0,
            scan: 0x1E,
            flags: KEY_SCANCODE
        })
    );
    assert_eq!(
        key(Key::ArrowLeft.evdev().unwrap(), false),
        Some(Synth::Key {
            vk: 0,
            scan: 0x4B,
            flags: KEY_SCANCODE | KEY_EXTENDED | KEY_UP
        })
    );
    assert_eq!(
        key(Key::VolumeUp.evdev().unwrap(), true),
        Some(Synth::Key {
            vk: 0xAF,
            scan: 0,
            flags: 0
        })
    );
    assert_eq!(key(Key::MicMute.evdev().unwrap(), true), None);
}
