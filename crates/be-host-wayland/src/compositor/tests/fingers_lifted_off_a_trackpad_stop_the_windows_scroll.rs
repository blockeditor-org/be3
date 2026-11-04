use super::*;
use wayland_client::protocol::wl_pointer::AxisSource;

#[test]
fn fingers_lifted_off_a_trackpad_stop_the_windows_scroll() {
    let mut harness = Harness::new();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let at = harness.window(id).center();
    harness.frame(vec![Input::PointerMoved(at)]);
    harness.settle();

    harness.frame(vec![
        Input::ScrollStarted,
        Input::Scroll(beui::ScrollDelta::Pixels(beui::vec2(0.0, -20.0))),
    ]);
    harness.settle();
    let received = &harness.client.received;
    assert_eq!(
        received.axis_sources,
        [AxisSource::Finger],
        "a slide between a start and an end is fingers, which clients pan by"
    );
    assert_eq!(received.scrolled, 20.0);
    assert_eq!(received.axis_stops, 0);

    harness.frame(vec![Input::ScrollEnded]);
    harness.settle();
    let received = &harness.client.received;
    assert_eq!(
        received.axis_stops, 2,
        "lifting the fingers stops both axes, so the client can fling"
    );
    assert_eq!(
        received.axis_sources,
        [AxisSource::Finger, AxisSource::Finger]
    );
}
