use super::*;

#[test]
fn a_repositioned_popup_is_configured_again() {
    let mut harness = Harness::new();
    let (window, _id) = harness.open();
    let menu = harness.client.popup(
        harness.stepped.app_mut().compositor.server(),
        &window,
        (10, 10),
        (150, 200),
    );
    harness.settle();
    harness.client.received.configured = None;

    harness.client.reposition(
        harness.stepped.app_mut().compositor.server(),
        &menu,
        (20, 30),
        (180, 90),
        7,
    );
    assert_eq!(harness.client.received.repositioned, Some(7));
    assert!(
        harness.client.received.configured.is_some(),
        "repositioned is followed by a configure the client can ack"
    );
    assert_eq!(
        harness.client.received.popup_geometry,
        Some((20, 30, 180, 90))
    );
}
