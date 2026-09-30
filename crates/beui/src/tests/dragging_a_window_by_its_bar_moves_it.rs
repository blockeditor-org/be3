use super::*;
use crate::unstyled::dock_state;

#[test]
fn dragging_a_window_by_its_bar_moves_it() {
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
    let origin = harness.rect(dock).min + before.min.to_vec2();

    let bar = pos2(origin.x + before.width() - 80.0, origin.y + 10.0);
    harness.drag(bar, bar + vec2(-60.0, -40.0));
    harness.frame(Vec::new());

    let after = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(
        after.min,
        before.min + vec2(-60.0, -40.0),
        "dragging the empty part of a window's bar moves the window by what the pointer moved"
    );
    assert_eq!(
        after.size(),
        before.size(),
        "moving a window leaves its size alone"
    );
}
