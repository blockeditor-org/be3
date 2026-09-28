use super::*;
use crate::unstyled::dock_state;

#[test]
fn a_window_released_past_the_edge_of_the_dock_springs_back_and_comes_to_rest() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let window = floated_window(&mut harness, dock);
    let before = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window has a rect");
    let bounds = harness.rect(dock);
    let drawn_left = |harness: &Harness| {
        harness
            .rect(dock_tab(harness.document(), dock, "Tab 2"))
            .min
            .x
            - bounds.min.x
    };
    let inset = drawn_left(&harness) - before.min.x;

    let bar = bounds.min + before.min.to_vec2() + vec2(before.width() - 80.0, 10.0);
    let pulled = bar - vec2(before.min.x + 200.0, 0.0);
    harness.press_at(bar);
    harness.frame(vec![Event::PointerMoved(pulled)]);
    harness.advance(Duration::from_millis(200));
    harness.release_at(pulled);
    let released = drawn_left(&harness) - inset;
    assert!(
        released < 0.0,
        "the window starts out stretched: {released}"
    );

    harness.frame(Vec::new());
    let returning = drawn_left(&harness) - inset;
    assert!(
        returning > released && returning < 0.0,
        "one frame later the window has moved part of the way back: {released} -> {returning}"
    );

    let frames = harness.settle();
    assert!(
        frames > 5 && frames < 120,
        "the spring takes a visible but short time: {frames} frames"
    );
    assert_eq!(
        drawn_left(&harness) - inset,
        0.0,
        "the window comes to rest exactly against the edge"
    );
    assert_eq!(
        dock_state(harness.document(), dock)
            .window_rect(window)
            .expect("the window is still open")
            .min
            .x,
        0.0
    );
}
