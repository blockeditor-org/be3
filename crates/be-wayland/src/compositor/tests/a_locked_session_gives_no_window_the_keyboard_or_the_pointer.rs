use super::*;

const KEY_A: u32 = 30;
const KEY_B: u32 = 48;

#[test]
fn a_locked_session_gives_no_window_the_keyboard_or_the_pointer() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let window = format!("wayland.window.{}", id.0);

    harness.click(&window);
    harness.key(KEY_A);
    assert!(harness.client.received.keyboard_entered);
    assert!(harness.client.received.pointer_surface.is_some());
    assert_eq!(
        harness.client.received.keys,
        [(KEY_A, true), (KEY_A, false)]
    );

    harness.lock(true);
    assert!(
        !harness.client.received.keyboard_entered,
        "locking takes the keyboard from the window"
    );
    assert!(
        harness.client.received.pointer_surface.is_none(),
        "and the pointer"
    );
    assert!(!harness.client.received.activated);

    let buttons = harness.client.received.buttons.len();
    harness.click(&window);
    harness.key(KEY_B);
    harness.frame(vec![Event::Scroll(beui::vec2(0.0, 40.0))]);
    harness.app.windows().activate(id);
    harness.settle();
    assert_eq!(
        harness.client.received.keys,
        [(KEY_A, true), (KEY_A, false)],
        "no key reaches a window while locked, though it was clicked"
    );
    assert_eq!(harness.client.received.buttons.len(), buttons);
    assert!(harness.client.received.pointer_surface.is_none());
    assert!(!harness.client.received.keyboard_entered);
    assert_eq!(harness.client.received.scrolled, 0.0);

    harness.lock(false);
    harness.click(&window);
    harness.key(KEY_B);
    assert_eq!(
        harness.client.received.keys,
        [(KEY_A, true), (KEY_A, false), (KEY_B, true), (KEY_B, false)],
        "once unlocked the window takes keys again"
    );
}
