use super::*;

const KEY_A: u32 = 30;

#[test]
fn a_window_whose_first_focus_was_refused_gets_the_keyboard_when_raised() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    harness.ask(true);
    let (_window, id) = harness.open();
    harness.ask(false);
    assert!(
        !harness.client.received.keyboard_entered,
        "the window opened under a dialog did not get the keyboard"
    );

    harness.compositor().windows().activate(id);
    harness.settle();
    assert!(
        harness.client.received.keyboard_entered,
        "raising the window gives it the keyboard"
    );
    harness.key(KEY_A);
    assert_eq!(
        harness.client.received.keys,
        [(KEY_A, true), (KEY_A, false)]
    );
}
