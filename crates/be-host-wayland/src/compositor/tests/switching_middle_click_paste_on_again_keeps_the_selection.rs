use super::*;
use crate::selection::client;

#[test]
fn switching_middle_click_paste_on_again_keeps_the_selection() {
    let mut harness = Harness::new();
    harness.compositor().set_primary_selection(true);
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let device = harness.client.primary_device();
    let (_window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));
    let serial = harness
        .client
        .received
        .serial
        .expect("the click carried a serial");
    harness
        .client
        .select(&device, "selected in a window", serial);
    harness.settle();
    assert!(client::primary().is_some());

    // Every desktop setting is handed over whole when any one changes, so this comes again.
    harness.compositor().set_primary_selection(true);
    harness.settle();
    assert!(
        !client::primary_cancelled(),
        "a setting that did not change leaves the selection alone"
    );
    assert!(
        client::primary().is_some(),
        "the window is still offered it"
    );
}
