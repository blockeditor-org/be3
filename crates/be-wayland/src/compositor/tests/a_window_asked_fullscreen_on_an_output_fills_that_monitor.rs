use super::*;

#[test]
fn a_window_asked_fullscreen_on_an_output_fills_that_monitor() {
    let mut harness = Harness::new();
    let left = Rect::from_min_size(Pos2::ZERO, Vec2::new(SCREEN.x / 2.0, SCREEN.y));
    let right = Rect::from_min_max(pos2(SCREEN.x / 2.0, 0.0), pos2(SCREEN.x, SCREEN.y));
    harness.compositor().set_monitors(monitors(&[left, right]));
    harness.settle();
    harness.client.bind_outputs();
    harness.settle();
    let (window, _) = harness.open();
    let output = harness
        .client
        .output_named("monitor 1")
        .expect("the client bound the right monitor's output");

    window
        .toplevel
        .as_ref()
        .unwrap()
        .set_fullscreen(Some(&output));
    harness.acknowledge(&window);
    let listed = harness.compositor().windows().list().get_untracked();
    assert_eq!(
        listed[0].fullscreen,
        Some(right),
        "the window fills the monitor it asked for, not the one it was shown on"
    );
}
