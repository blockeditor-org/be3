use super::*;

#[test]
fn a_window_asking_for_fullscreen_before_it_is_shown_is_answered() {
    let mut harness = Harness::new();
    let window = harness.client.toplevel();
    harness.settle();
    let serial = harness
        .client
        .received
        .configured
        .take()
        .expect("the first commit is configured");
    window.xdg_surface.ack_configure(serial);
    harness.settle();

    window.toplevel.as_ref().unwrap().set_fullscreen(None);
    harness.settle();
    assert!(
        harness.client.received.configured.is_some(),
        "a window that drew nothing yet is still answered"
    );
    assert!(harness.client.received.fullscreen);
    assert_eq!(
        harness.client.received.size,
        Some((SCREEN.x as i32, SCREEN.y as i32)),
        "with the size of the screen it will cover"
    );
}
