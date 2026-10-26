use super::*;
use crate::state::PING_TIMEOUT;

const KEY_A: u32 = 30;
const KEY_B: u32 = 48;

#[test]
fn a_window_no_shell_owns_gets_no_input_and_is_still_pinged() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let window = format!("wayland.window.{}", id.0);
    harness.click(&window);
    harness.key(KEY_A);
    assert!(harness.client.received.keyboard_entered);
    assert!(harness.client.received.pointer_surface.is_some());

    harness.compositor().set_owned(false);
    harness.settle();
    assert!(
        !harness.client.received.keyboard_entered,
        "a window no shell owns loses the keyboard"
    );
    assert!(
        harness.client.received.pointer_surface.is_none(),
        "and the pointer"
    );
    assert!(!harness.client.received.activated);

    let buttons = harness.client.received.buttons.len();
    harness.click(&window);
    harness.key(KEY_B);
    harness.compositor().windows().activate(id);
    harness.settle();
    assert_eq!(
        harness.client.received.keys,
        [(KEY_A, true), (KEY_A, false)],
        "no key reaches it"
    );
    assert_eq!(harness.client.received.buttons.len(), buttons);
    assert!(!harness.client.received.keyboard_entered);

    harness.client.received.ignores_pings = true;
    harness.client.received.pings.clear();
    harness.context().advance_clock(PING_TIMEOUT);
    harness.settle();
    assert!(
        !harness.client.received.pings.is_empty(),
        "its client is still pinged though nothing is pressed"
    );
    assert!(
        harness.output.as_ref().unwrap().repaint_after <= PING_TIMEOUT,
        "the compositor wakes for the next ping"
    );
    harness.context().advance_clock(PING_TIMEOUT);
    harness.settle();
    assert!(
        !harness.responding(id),
        "a client that stops answering is found not responding"
    );
    harness.client.received.ignores_pings = false;
    harness.client.pong();
    harness.settle();
    assert!(harness.responding(id));
    assert!(
        harness.compositor().server().state.windows().contains(&id),
        "the window stays open the whole time"
    );

    harness.compositor().set_owned(true);
    harness.click(&window);
    harness.key(KEY_B);
    assert!(harness.client.received.keyboard_entered);
    assert_eq!(
        harness.client.received.keys,
        [(KEY_A, true), (KEY_A, false), (KEY_B, true), (KEY_B, false)],
        "once a shell owns it again it takes keys"
    );
}
