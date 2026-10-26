use super::*;
use crate::activation::TOKEN_LIFETIME;

#[test]
fn a_stale_or_foreign_token_does_not_take_the_focus() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (first, first_id) = harness.open();
    let (second, _) = harness.open();

    harness.click(&format!("wayland.window.{}", first_id.0));
    let serial = harness.client.received.serial;
    harness.client.request_token(serial, Some(&first.surface));
    harness.settle();
    let stale = harness.client.received.token.take().expect("a token");
    harness.context().advance_clock(TOKEN_LIFETIME);
    harness.settle();
    harness.client.activate(&stale, &second.surface);
    harness.settle();
    assert_eq!(
        harness.compositor().windows().focused(),
        Some(first_id),
        "a token that has run out does not move the focus"
    );

    harness.click("test.input");
    assert_eq!(harness.compositor().windows().focused(), None);
    harness.client.request_token(serial, Some(&first.surface));
    harness.settle();
    let foreign = harness.client.received.token.take().expect("a token");
    harness.client.activate(&foreign, &second.surface);
    harness.settle();
    assert_eq!(
        harness.compositor().windows().focused(),
        None,
        "a client without the keyboard cannot make a token that takes it"
    );
    assert!(!harness.client.received.keyboard_entered);

    harness.client.activate("made-up", &second.surface);
    harness.settle();
    assert_eq!(
        harness.compositor().windows().focused(),
        None,
        "nor can a made-up one"
    );
}
