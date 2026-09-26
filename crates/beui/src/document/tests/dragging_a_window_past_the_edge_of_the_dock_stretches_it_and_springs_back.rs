use super::*;
use crate::unstyled::dock_state;

#[test]
fn dragging_a_window_past_the_edge_of_the_dock_stretches_it_and_springs_back() {
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
    let past = 200.0;
    let pulled = bar - vec2(before.min.x + past, 0.0);
    harness.press_at(bar);
    harness.frame(vec![Event::PointerMoved(pulled)]);

    let held = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(held.min.x, 0.0, "the window rests against the left edge");
    let stretched = drawn_left(&harness) - inset;
    assert!(
        stretched < 0.0 && stretched > -past,
        "the window is drawn past the edge, but by less than the pointer went: {stretched}"
    );

    let released = harness.release_at(pulled);

    let rested = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(
        rested.min.x, 0.0,
        "releasing leaves the window against the edge it was pulled past"
    );
    assert!(
        drawn_left(&harness) - inset < 0.0,
        "the window springs back from where it was released rather than jumping"
    );
    assert!(
        released.repaint || released.repaint_after == Duration::ZERO,
        "the spring back keeps asking for frames"
    );
}
