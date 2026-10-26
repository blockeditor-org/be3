use super::*;

#[test]
fn a_frame_callback_waits_for_the_flip_of_the_screen_the_window_is_on() {
    let (mut harness, device, queue) = Harness::with_gpu();
    harness.compositor().pace_to_flips();
    let left = Rect::from_min_size(Pos2::ZERO, Vec2::new(SCREEN.x / 2.0, SCREEN.y));
    let right = Rect::from_min_max(pos2(SCREEN.x / 2.0, 0.0), pos2(SCREEN.x, SCREEN.y));
    harness.compositor().set_monitors(monitors(&[left, right]));
    let (window, id) = harness.open();
    assert!(
        left.contains(harness.window(id).center()),
        "the window is shown on the left screen"
    );
    let mut painter = Painter::new(&device, &queue);

    window.surface.frame(&harness.client.handle, ());
    harness.client.attach_unsent(&window, 300, 200);
    harness.frame(Vec::new());
    painter.paint(&harness);
    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.frames, 0,
        "a painted window is not told to draw again before its screen flips"
    );
    assert!(
        harness.compositor().awaits_flip(),
        "the host is asked for the flip"
    );

    harness.compositor().presented(right, flip(1));
    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.frames, 0,
        "another screen's flip does not answer it"
    );

    harness.compositor().presented(left, flip(2));
    harness.frame(Vec::new());
    assert_eq!(harness.client.received.frames, 1);
    assert!(!harness.compositor().awaits_flip());
}
