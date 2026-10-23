use super::*;

#[test]
fn a_token_is_lost_once_the_keyboard_leaves_its_window() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (first, first_id) = harness.open();
    let (second, _) = harness.open();

    harness.click(&format!("wayland.window.{}", first_id.0));
    let serial = harness.client.received.serial;
    harness.client.request_token(serial, Some(&first.surface));
    harness.settle();
    let token = harness.client.received.token.take().expect("a token");

    harness.click("test.input");
    harness.click(&format!("wayland.window.{}", first_id.0));
    assert_eq!(harness.compositor().windows().focused(), Some(first_id));
    harness.client.activate(&token, &second.surface);
    harness.settle();
    assert_eq!(
        harness.compositor().windows().focused(),
        Some(first_id),
        "the keyboard came back to the window, but the token went when it left"
    );
}
