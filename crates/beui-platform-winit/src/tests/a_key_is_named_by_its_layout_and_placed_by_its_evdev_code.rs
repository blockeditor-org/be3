use super::*;
use winit::keyboard::{Key as Winit, KeyCode, NamedKey, NativeKey};

#[test]
fn a_key_is_named_by_its_layout_and_placed_by_its_evdev_code() {
    assert_eq!(evdev(PhysicalKey::Code(KeyCode::KeyQ)), Some(16));
    assert_eq!(evdev(PhysicalKey::Code(KeyCode::ShiftRight)), Some(54));
    assert_eq!(evdev(PhysicalKey::Code(KeyCode::Numpad1)), Some(79));
    assert_eq!(
        evdev(PhysicalKey::Unidentified(NativeKeyCode::Xkb(158 + 8))),
        Some(158),
        "an xkb keycode is the evdev code plus eight"
    );
    assert_eq!(
        evdev(PhysicalKey::Unidentified(NativeKeyCode::Unidentified)),
        None
    );
    let at = |code| evdev(PhysicalKey::Code(code)).and_then(Key::from_evdev);
    for (text, code, key) in [
        ("a", KeyCode::KeyQ, Key::A),
        ("&", KeyCode::Digit1, Key::One),
        ("ě", KeyCode::Digit2, Key::Two),
        ("с", KeyCode::KeyC, Key::C),
        (",", KeyCode::KeyM, Key::M),
    ] {
        assert_eq!(
            logical_key(&Winit::Character(text.into()), at(code)),
            Some(key),
            "{text}: a letter or digit names its key, anything else falls to its place"
        );
    }
    assert_eq!(
        logical_key(&Winit::Dead(None), at(KeyCode::BracketLeft)),
        Some(Key::BracketLeft)
    );
    assert_eq!(
        logical_key(
            &Winit::Unidentified(NativeKey::Unidentified),
            Key::from_evdev(158)
        ),
        Some(Key::BrowserBack)
    );
    assert_eq!(
        logical_key(&Winit::Named(NamedKey::CapsLock), at(KeyCode::CapsLock)),
        None
    );
}
