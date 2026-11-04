use super::*;
use winit::keyboard::{Key as Winit, KeyCode, NamedKey};

#[test]
fn the_context_menu_key_is_named_and_placed() {
    let placed = evdev(PhysicalKey::Code(KeyCode::ContextMenu));
    assert_eq!(placed, Some(127));
    assert_eq!(
        logical_key(
            &Winit::Named(NamedKey::ContextMenu),
            placed.and_then(Key::from_evdev)
        ),
        Some(Key::ContextMenu)
    );
}
