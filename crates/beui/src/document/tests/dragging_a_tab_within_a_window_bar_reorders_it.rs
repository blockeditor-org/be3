use super::*;
use crate::unstyled::{TabId, dock_state};

#[test]
fn dragging_a_tab_within_a_window_bar_reorders_it() {
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
    let rect = state.window_rect(window).expect("the window has a rect");
    let origin = harness.rect(dock).min + rect.min.to_vec2();
    let floated = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    harness.drag(tab, pos2(floated.right() + 20.0, origin.y + 10.0));
    harness.frame(Vec::new());

    let first = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    let second = harness.rect(dock_tab(harness.document(), dock, "Tab 1"));
    harness.drag(first, pos2(second.right() + 4.0, second.center().y));
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    assert_eq!(
        state.surface_tabs(window),
        vec![TabId::new(1), TabId::new(2)],
        "a tab dragged past the one beside it in a window's bar changes places with it"
    );
    assert_eq!(
        state.window_rect(window),
        Some(rect),
        "dragging a tab in a window's bar does not move the window"
    );
}
