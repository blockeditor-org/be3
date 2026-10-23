use super::*;

#[test]
fn a_token_from_a_click_in_the_focused_window_brings_another_window_forward() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (first, first_id) = harness.open();
    let (second, second_id) = harness.open();

    harness.click(&format!("wayland.window.{}", first_id.0));
    assert_eq!(harness.compositor().windows().focused(), Some(first_id));
    let serial = harness.client.received.serial;
    assert!(serial.is_some(), "the click carried a serial");

    harness.client.request_token(serial, Some(&first.surface));
    harness.settle();
    let token = harness
        .client
        .received
        .token
        .take()
        .expect("the client is given a token");

    harness.client.activate(&token, &second.surface);
    harness.settle();
    assert_eq!(
        harness.compositor().windows().focused(),
        Some(second_id),
        "the token brings the other window forward"
    );
    assert_eq!(
        harness.client.received.keyboard_surface.as_ref(),
        Some(&second.surface),
        "and gives it the keyboard"
    );

    harness.click(&format!("wayland.window.{}", first_id.0));
    harness.client.activate(&token, &second.surface);
    harness.settle();
    assert_eq!(
        harness.compositor().windows().focused(),
        Some(first_id),
        "a token is used up once it has been honoured"
    );
}
