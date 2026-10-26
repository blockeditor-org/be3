use super::*;

#[test]
fn a_window_on_no_screen_has_its_presentation_discarded() {
    let (mut harness, device, queue) = Harness::with_gpu();
    harness.compositor().pace_to_flips();
    let right = Rect::from_min_max(pos2(SCREEN.x / 2.0, 0.0), pos2(SCREEN.x, SCREEN.y));
    harness.compositor().set_monitors(monitors(&[right]));
    let (window, id) = harness.open();
    assert!(
        !right.contains(harness.window(id).center()),
        "the window is shown off the only screen"
    );
    let mut painter = Painter::new(&device, &queue);

    let _feedback = harness.client.presentation_feedback(&window.surface);
    window.surface.frame(&harness.client.handle, ());
    harness.client.attach_unsent(&window, 300, 200);
    harness.frame(Vec::new());
    painter.paint(&harness);
    harness.compositor().presented(right, flip(1));
    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.presented,
        Vec::new(),
        "no screen's flip showed it"
    );
    assert_eq!(harness.client.received.discarded, 1);
    assert_eq!(
        harness.client.received.frames, 1,
        "it is still told when to draw again"
    );
}
