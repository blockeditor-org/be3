use super::*;
use crate::test_client::PointerSeen;
use beui::vec2;

#[test]
fn super_escape_lets_go_of_a_locked_pointer_until_its_window_is_clicked() {
    let mut harness = Harness::new();
    let pointer = harness.client.pointer();
    let _relative = harness.client.relative_pointer(&pointer);
    let _keyboard = harness.client.keyboard();
    let (window, id) = harness.open();
    let target = format!("wayland.window.{}", id.0);
    harness.click(&target);
    let _locked = harness.client.lock_pointer(&window.surface, &pointer);
    harness.settle();

    let escape = |pressed: bool| vec![stroke(Some(Key::Escape), KEY_ESC, pressed)];
    harness.frame(
        [
            super_key(true),
            escape(true),
            escape(false),
            super_key(false),
        ]
        .concat(),
    );
    harness.settle();
    harness.frame(vec![Input::PointerMotion(vec2(5.0, 0.0))]);
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.pointer,
        [PointerSeen::Locked, PointerSeen::Unlocked],
        "Super+Escape takes the pointer back, and raw motion no longer reaches the window"
    );
    assert_eq!(
        harness.compositor().keyboard_window().map(|info| info.id),
        Some(id),
        "the window keeps the keyboard"
    );

    harness.client.received.protocols.pointer.clear();
    harness.click(&target);
    assert_eq!(
        harness.client.received.protocols.pointer,
        [PointerSeen::Locked],
        "clicking the window gives it its lock again"
    );
}
