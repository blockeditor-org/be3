use super::*;
use crate::unstyled::dock_state;

#[test]
fn a_window_can_be_dragged_partly_off_screen_but_not_out_of_reach() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    drag_with(
        &mut harness,
        tab,
        pos2(WIDE_VIEWPORT.x / 2.0, WIDE_VIEWPORT.y / 2.0),
        Modifiers::ALT,
    );
    harness.frame(Vec::new());
    let state = dock_state(harness.document(), dock);
    let window = state.windows()[0];
    let before = state.window_rect(window).expect("the window has a rect");
    let bounds = harness.rect(dock);
    let origin = bounds.min + before.min.to_vec2();

    let bar = pos2(origin.x + before.width() - 80.0, origin.y + 10.0);
    let past = vec2(-before.width() / 2.0 - before.min.x, 0.0);
    harness.drag(bar, bar + past);
    harness.frame(Vec::new());

    let partly = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(
        partly.min.x,
        -before.width() / 2.0,
        "a window can be dragged past the left edge of the dock"
    );
    assert!(
        harness
            .rect(dock_tab(harness.document(), dock, "Tab 2"))
            .min
            .x
            < bounds.min.x,
        "the window is drawn where it was dragged to, not pulled back on screen"
    );

    let bar = bounds.min + partly.min.to_vec2() + vec2(partly.width() - 80.0, 10.0);
    harness.drag(
        bar,
        bar + vec2(-4.0 * bounds.width(), 4.0 * bounds.height()),
    );
    harness.frame(Vec::new());

    let reachable = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window is still open");
    assert!(
        reachable.max.x > 0.0 && reachable.min.y < bounds.height(),
        "a window dragged far away keeps part of its bar inside the dock: {reachable:?}"
    );

    let grab = bounds.min + reachable.min.to_vec2() + vec2(reachable.width() - 60.0, 10.0);
    harness.drag(grab, grab + vec2(200.0, -200.0));
    harness.frame(Vec::new());

    let back = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(
        back.min,
        reachable.min + vec2(200.0, -200.0),
        "the part of the bar left inside the dock still drags the window"
    );
}
