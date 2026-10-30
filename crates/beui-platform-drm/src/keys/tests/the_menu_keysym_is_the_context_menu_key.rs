use super::*;

#[test]
fn the_menu_keysym_is_the_context_menu_key() {
    const KEY_COMPOSE: u32 = 127;
    assert_eq!(key(keysyms::KEY_Menu, KEY_COMPOSE), Some(Key::ContextMenu));
    assert_eq!(
        key(keysyms::KEY_Menu, 0),
        Some(Key::ContextMenu),
        "the keysym names it wherever the key sits"
    );
}
