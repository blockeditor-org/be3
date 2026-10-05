use super::*;

#[test]
fn keysyms_map_to_the_keys_beui_knows() {
    assert_eq!(key(keysyms::KEY_a), Some(Key::A));
    assert_eq!(
        key(keysyms::KEY_Q),
        Some(Key::Q),
        "shifted letters are the same key"
    );
    assert_eq!(key(keysyms::KEY_exclam), Some(Key::One));
    assert_eq!(key(keysyms::KEY_F12), Some(Key::F12));
    assert_eq!(key(keysyms::KEY_KP_Enter), Some(Key::Enter));
    assert_eq!(key(keysyms::KEY_ISO_Left_Tab), Some(Key::Tab));
    assert_eq!(key(keysyms::KEY_Shift_L), None, "modifiers are not keys");
}
