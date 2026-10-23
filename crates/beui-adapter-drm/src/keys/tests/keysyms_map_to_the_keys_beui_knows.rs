use super::*;

#[test]
fn keysyms_map_to_the_keys_beui_knows() {
    const KEY_Q: u32 = 16;
    const KEY_1: u32 = 2;
    const KEY_SEMICOLON: u32 = 39;
    assert_eq!(key(keysyms::KEY_a, KEY_Q), Some(Key::A));
    assert_eq!(
        key(keysyms::KEY_A, KEY_Q),
        Some(Key::A),
        "shifted letters are the same key"
    );
    assert_eq!(
        key(keysyms::KEY_exclam, KEY_1),
        Some(Key::One),
        "a symbol is the key in its place"
    );
    assert_eq!(
        key(keysyms::KEY_semicolon, KEY_SEMICOLON),
        Some(Key::Semicolon)
    );
    assert_eq!(key(keysyms::KEY_KP_1, 79), Some(Key::One));
    assert_eq!(key(keysyms::KEY_KP_Add, 78), Some(Key::Plus));
    assert_eq!(key(keysyms::KEY_F12, 88), Some(Key::F12));
    assert_eq!(key(keysyms::KEY_KP_Enter, 96), Some(Key::Enter));
    assert_eq!(key(keysyms::KEY_ISO_Left_Tab, 15), Some(Key::Tab));
    assert_eq!(key(keysyms::KEY_Shift_L, 42), Some(Key::Shift));
    assert_eq!(key(keysyms::KEY_Super_R, 126), Some(Key::Logo));
    assert_eq!(key(keysyms::KEY_Caps_Lock, 58), None, "locks are not keys");
}
