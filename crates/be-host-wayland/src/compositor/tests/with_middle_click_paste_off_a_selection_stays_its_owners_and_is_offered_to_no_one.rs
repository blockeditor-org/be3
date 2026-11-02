use super::*;
use crate::selection::client;

#[test]
fn with_middle_click_paste_off_a_selection_stays_its_owners_and_is_offered_to_no_one() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let device = harness.client.primary_device();
    let control = harness.client.data_control();
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
    assert!(
        !client::primary_cancelled(),
        "the window keeps its selection, so a terminal keeps it highlighted"
    );
    assert!(client::primary().is_none(), "the window is offered nothing");
    assert!(
        client::control_primary().is_none(),
        "a clipboard manager is offered nothing"
    );

    harness
        .client
        .select_with_control(&control, "selected by a manager");
    harness.settle();
    assert!(
        !client::source_cancelled(),
        "a clipboard manager keeps its selection too"
    );
    assert!(client::primary().is_none());
    assert!(client::control_primary().is_none());
}
