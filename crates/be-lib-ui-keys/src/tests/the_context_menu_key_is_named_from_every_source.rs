use super::*;

#[test]
fn the_context_menu_key_is_named_from_every_source() {
    assert_eq!(
        Key::from_value("ContextMenu", None, false),
        Some(Key::ContextMenu),
        "the W3C key value, which the web and winit report"
    );
    assert_eq!(Key::evdev_from_code("ContextMenu"), Some(127));
    assert_eq!(Key::ContextMenu.evdev(), Some(127), "KEY_COMPOSE");
    assert_eq!(Key::from_evdev(127), Some(Key::ContextMenu));
    assert_eq!(
        Key::from_evdev(139),
        Some(Key::ContextMenu),
        "KEY_MENU, which some keyboards send for it"
    );
    for name in ["ContextMenu", "Menu", "apps"] {
        assert_eq!(Key::from_name(name), Some(Key::ContextMenu), "{name}");
    }
    assert_eq!(Key::ContextMenu.name().as_deref(), Some("ContextMenu"));
    assert!(!Key::ContextMenu.is_modifier() && !Key::ContextMenu.is_media());
}
