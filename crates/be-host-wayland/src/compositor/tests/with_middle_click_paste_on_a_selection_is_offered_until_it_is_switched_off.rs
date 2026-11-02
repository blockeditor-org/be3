use super::*;
use crate::selection::client;

#[test]
fn with_middle_click_paste_on_a_selection_is_offered_until_it_is_switched_off() {
    let mut harness = Harness::new();
    harness.compositor().set_primary_selection(true);
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let device = harness.client.primary_device();
    let _control = harness.client.data_control();
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
    assert!(!client::primary_cancelled());
    let offer = client::primary().expect("the window is offered its selection");
    assert!(client::mime_types(&offer).contains(&client::TEXT.to_owned()));
    let offer = client::control_primary().expect("a clipboard manager is offered it");
    let answer = client::ask(&offer);
    harness.settle();
    assert_eq!(client::answer(answer), "selected in a window");

    harness.compositor().set_primary_selection(false);
    harness.settle();
    assert!(
        client::primary_cancelled(),
        "switching it off cancels the selection"
    );
    assert!(client::primary().is_none(), "the window is offered nothing");
    assert!(client::control_primary().is_none());
}
