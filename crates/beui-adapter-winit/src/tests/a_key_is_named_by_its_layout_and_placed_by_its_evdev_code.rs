use super::*;
use winit::keyboard::{Key as Winit, KeyCode};

#[test]
fn a_key_is_named_by_its_layout_and_placed_by_its_evdev_code() {
    assert_eq!(evdev(PhysicalKey::Code(KeyCode::KeyQ)), Some(16));
    assert_eq!(evdev(PhysicalKey::Code(KeyCode::ShiftRight)), Some(54));
    assert_eq!(evdev(PhysicalKey::Code(KeyCode::Numpad1)), Some(79));
    assert_eq!(
        evdev(PhysicalKey::Unidentified(NativeKeyCode::Xkb(EVDEV_BACK + 8))),
        Some(EVDEV_BACK),
        "an xkb keycode is the evdev code plus eight"
    );
    assert_eq!(
        evdev(PhysicalKey::Unidentified(NativeKeyCode::Unidentified)),
        None
    );
    assert!(
        matches!(
            logical_key(&Winit::Character("a".into())),
            Logical::Key(Key::A)
        ),
        "the key that types a is A, wherever the layout puts it"
    );
    assert!(matches!(
        logical_key(&Winit::Character("ф".into())),
        Logical::Unknown
    ));
}
