use super::*;
use crate::unstyled::dock_state;

#[test]
fn a_secondary_drag_with_the_dock_drag_modifier_resizes_a_window_from_its_nearest_corner() {
    let SplitDock {
        document,
        dock,
        presses,
    } = split_dock(Some(Modifiers::LOGO));
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    drag_with(
        &mut harness,
        tab,
        pos2(WIDE_VIEWPORT.x / 2.0, WIDE_VIEWPORT.y / 2.0),
        Modifiers::ALT,
    );
    harness.frame(Vec::new());
    let state = dock_state(harness.document(), dock);
    let window = state.windows()[0];
    let before = state
        .window_rect(window)
        .expect("the tab floats in a window");

    let content = harness.rect(harness.find("content.1"));
    let corner = content.max - vec2(8.0, 8.0);
    secondary_drag_with(
        &mut harness,
        corner,
        corner + vec2(-60.0, -30.0),
        Modifiers::LOGO,
    );
    harness.frame(Vec::new());

    let after = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window is still open");
    assert_eq!(after.min, before.min, "the far corner stays where it was");
    assert_eq!(
        after.size(),
        before.size() - vec2(60.0, 30.0),
        "the corner nearest the pointer follows it"
    );
    assert_eq!(presses.get(), 0);
}
