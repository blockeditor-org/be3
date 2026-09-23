use super::*;

const KEY_A: u32 = 30;
const KEY_B: u32 = 48;

#[test]
fn keys_follow_the_focus_between_beui_and_a_window() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, _id) = harness.open();

    harness.click("compositor.command");
    harness.frame(vec![Event::PointerMoved(outside())]);
    harness.key(KEY_A);
    assert!(
        harness.client.received.keys.is_empty(),
        "a key typed into the launcher stays in beui"
    );
    assert!(!harness.client.received.activated);

    harness.click("compositor.client.1");
    assert!(
        harness.client.received.pointer_entered.is_some(),
        "the pointer entered the window it was clicked on"
    );
    assert!(
        harness.client.received.buttons.contains(&(0x110, true)),
        "the window got the press"
    );
    assert!(harness.client.received.activated);
    harness.key(KEY_B);
    assert_eq!(
        harness.client.received.keys,
        [(KEY_B, true), (KEY_B, false)],
        "the clicked window gets the keys, and only after it was clicked"
    );
}
