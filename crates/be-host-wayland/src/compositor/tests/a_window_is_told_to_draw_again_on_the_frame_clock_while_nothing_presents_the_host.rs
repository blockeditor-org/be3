use super::*;

#[test]
fn a_window_is_told_to_draw_again_on_the_frame_clock_while_nothing_presents_the_host() {
    let mut harness = Harness::new();
    let (window, _) = harness.open();

    window.surface.frame(&harness.client.handle, ());
    harness.client.attach_unsent(&window, 300, 200);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.frames, 0,
        "a seen host answers a frame callback once it paints the window"
    );

    harness.stepped.runner().set_occluded(true);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.frames, 1,
        "a host nothing presents, such as a headless one, answers it on its own frame"
    );

    window.surface.frame(&harness.client.handle, ());
    harness.client.attach_unsent(&window, 300, 200);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.client.received.frames, 2,
        "and keeps answering each frame the window draws"
    );
}
