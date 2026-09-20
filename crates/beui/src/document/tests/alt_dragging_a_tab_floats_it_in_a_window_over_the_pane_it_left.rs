use super::*;
use crate::unstyled::{TabId, dock_state};

#[test]
fn alt_dragging_a_tab_floats_it_in_a_window_over_the_pane_it_left() {
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
    let windows = state.windows();
    assert_eq!(
        windows.len(),
        1,
        "a tab dragged out with alt held is floated in a window of its own"
    );
    let window = windows[0];
    assert_eq!(
        state.surface_tabs(window),
        vec![TabId::new(2)],
        "the floated tab is the one that was dragged"
    );
    let floated = state.leaves(window)[0];
    assert_eq!(
        state.focused_leaf(),
        Some(floated),
        "the window that was just made has the focus"
    );

    let over = harness.rect(harness.find("content.2")).center();
    harness.click(over);
    harness.frame(Vec::new());

    assert_eq!(
        dock_state(harness.document(), dock).focused_leaf(),
        Some(floated),
        "a press inside the window does not reach the pane underneath it"
    );
}
