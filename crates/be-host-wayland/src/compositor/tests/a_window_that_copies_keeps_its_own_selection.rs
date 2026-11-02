use super::*;
use crate::selection::client;

#[test]
fn a_window_that_copies_keeps_its_own_selection() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let device = harness.client.data_device();
    let (_window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));
    let serial = harness
        .client
        .received
        .serial
        .expect("the click carried a serial");

    harness
        .client
        .copy_with_device(&device, "copied in a window", serial);
    let mut copied = None;
    for _ in 0..4 {
        harness.frame(Vec::new());
        copied = copied.or_else(|| harness.output.as_mut()?.copied_text.take());
    }
    assert_eq!(copied.as_deref(), Some("copied in a window"));
    harness.settle();

    assert!(
        !client::source_cancelled(),
        "the host's copy of the text does not replace the window's selection"
    );
    let offer = client::selection().expect("the window is offered its own selection");
    assert!(client::mime_types(&offer).contains(&"text/html".to_owned()));
}
