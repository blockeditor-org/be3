use super::*;
use wayland_client::protocol::wl_pointer::AxisSource;

#[test]
fn a_turned_wheel_reaches_a_window_as_wheel_notches_and_a_trackpad_as_a_slide() {
    let mut harness = Harness::new();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let at = harness.window(id).center();
    harness.frame(vec![Input::PointerMoved(at)]);
    harness.settle();

    harness.frame(vec![Input::Scroll(beui::ScrollDelta::Lines(beui::vec2(
        0.0, -1.0,
    )))]);
    harness.settle();
    let received = &harness.client.received;
    assert_eq!(received.axis_sources, [AxisSource::Wheel]);
    assert_eq!(received.notched, 1, "one notch down");
    assert_eq!(
        received.scrolled, 15.0,
        "a notch is libinput's 15 degrees, not the points beui scrolls for it"
    );

    harness.frame(vec![Input::Scroll(beui::ScrollDelta::Pixels(beui::vec2(
        0.0, -40.0,
    )))]);
    harness.settle();
    let received = &harness.client.received;
    assert_eq!(
        received.axis_sources,
        [AxisSource::Wheel, AxisSource::Continuous]
    );
    assert_eq!(received.notched, 1, "a slide turns no notches");
    assert_eq!(received.scrolled, 15.0 + 40.0);
}
