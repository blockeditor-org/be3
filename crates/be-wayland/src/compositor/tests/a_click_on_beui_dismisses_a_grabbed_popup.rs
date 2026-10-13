use crate::test_client::Seen;

use super::*;

#[test]
fn a_click_on_beui_dismisses_a_grabbed_popup() {
    let mut harness = Harness::new();
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));

    let menu = harness
        .client
        .grabbing_popup(harness.app.server(), &window, (0, 10), (40, 30));
    harness.settle();
    assert_eq!(
        harness.client.received.keyboard_surface.as_ref(),
        Some(&menu.surface)
    );

    harness.click("test.input");
    assert!(
        harness
            .client
            .received
            .seen
            .contains(&Seen::PopupDone(menu.popup.clone().unwrap())),
        "a press on the compositor's own UI dismisses the menu"
    );
}
