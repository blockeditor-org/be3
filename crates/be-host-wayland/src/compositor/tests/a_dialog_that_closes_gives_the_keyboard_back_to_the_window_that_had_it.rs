use super::*;

const KEY_A: u32 = 30;
const KEY_B: u32 = 48;
const KEY_C: u32 = 46;

#[test]
fn a_dialog_that_closes_gives_the_keyboard_back_to_the_window_that_had_it() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();

    harness.click(&format!("wayland.window.{}", id.0));
    harness.key(KEY_A);
    assert!(harness.client.received.keyboard_entered);

    harness.ask(true);
    assert!(
        !harness.client.received.keyboard_entered,
        "the dialog takes the keyboard from the window"
    );
    harness.key(KEY_B);

    harness.ask(false);
    assert!(
        harness.client.received.keyboard_entered,
        "closing the dialog gives the keyboard back to the window"
    );
    harness.key(KEY_C);
    assert_eq!(
        harness.client.received.keys,
        [(KEY_A, true), (KEY_A, false), (KEY_C, true), (KEY_C, false)],
        "the window gets the keys before and after the dialog, not while it is open"
    );
}
