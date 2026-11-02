use super::*;

fn scaled(rects: &[(Rect, f32)]) -> Vec<Monitor> {
    let mut monitors = monitors(&rects.iter().map(|(rect, _)| *rect).collect::<Vec<_>>());
    for (monitor, (_, scale)) in monitors.iter_mut().zip(rects) {
        monitor.scale = *scale;
    }
    monitors
}

#[test]
fn a_window_is_asked_for_the_fractional_scale_of_the_monitor_it_is_on() {
    let mut harness = Harness::new();
    let left = Rect::from_min_size(Pos2::ZERO, Vec2::new(SCREEN.x / 2.0, SCREEN.y));
    let right = Rect::from_min_max(pos2(SCREEN.x / 2.0, 0.0), pos2(SCREEN.x, SCREEN.y));
    harness
        .compositor()
        .set_monitors(scaled(&[(left, 1.5), (right, 1.25)]));
    harness.settle();
    let (window, id) = harness.open();
    assert!(left.contains_rect(harness.window(id)));
    let _fractional = harness.client.fractional_scale(&window.surface);
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.preferred_scale,
        Some(180),
        "a new fractional scale object is told its monitor's scale at once, in 120ths"
    );

    harness
        .compositor()
        .set_monitors(scaled(&[(left, 2.0), (right, 1.25)]));
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.preferred_scale,
        Some(240),
        "a monitor's new scale reaches the windows on it without waiting for a commit"
    );

    harness
        .compositor()
        .set_monitors(scaled(&[(right, 2.0), (left, 1.25)]));
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.preferred_scale,
        Some(150),
        "a window takes the scale of the monitor it is now on"
    );
}
