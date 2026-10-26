use super::*;

#[test]
fn a_window_that_moves_without_resizing_is_not_configured_again() {
    let mut harness = Harness::new();
    let mut other = TestClient::connect(harness.stepped.app_mut().compositor.server());
    let first = other.toplevel();
    other.flush();
    harness.settle();
    other.receive();
    let serial = other.received.configured.take().expect("it was configured");
    first.xdg_surface.ack_configure(serial);
    other.attach_unsent(&first, 300, 200);
    other.flush();
    harness.settle();
    let (second, id) = harness.open();
    harness.acknowledge(&second);
    let before = harness.window(id);
    harness.client.received.configured = None;

    first.toplevel.as_ref().unwrap().destroy();
    first.xdg_surface.destroy();
    other.flush();
    harness.settle();

    let after = harness.window(id);
    assert_ne!(after.min, before.min, "the window moved up into the gap");
    assert_eq!(after.size(), before.size());
    assert_eq!(
        harness.client.received.configured, None,
        "a move alone tells a Wayland client nothing"
    );
}
