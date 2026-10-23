use super::*;

#[test]
fn a_modifier_keystroke_knows_its_side_from_its_place() {
    let stroke = |key, code: &str| Keystroke {
        key: Some(key),
        pressed: true,
        evdev: Key::evdev_from_code(code),
        ..Keystroke::default()
    };
    for (key, code, side) in [
        (Key::Ctrl, "ControlLeft", KeySide::Left),
        (Key::Ctrl, "ControlRight", KeySide::Right),
        (Key::Shift, "ShiftLeft", KeySide::Left),
        (Key::Shift, "ShiftRight", KeySide::Right),
        (Key::Alt, "AltRight", KeySide::Right),
        (Key::Logo, "MetaLeft", KeySide::Left),
    ] {
        assert_eq!(stroke(key, code).side(), Some(side), "{code}");
        assert!(stroke(key, code).is(key, side), "{code}");
    }
    assert_eq!(
        stroke(Key::A, "KeyA").side(),
        None,
        "only a modifier has a side"
    );
    assert_eq!(
        Keystroke {
            key: Some(Key::Ctrl),
            ..Keystroke::default()
        }
        .side(),
        None,
        "a keystroke with no place has no side"
    );
}
