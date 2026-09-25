use super::*;
use crate::unstyled::{TabId, dock_state};

#[test]
fn dragging_a_tab_onto_a_window_bar_moves_it_into_the_window() {
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

    let state = dock_state(harness.document(), dock);
    assert_eq!(
        state.surface_tabs(window),
        vec![TabId::new(2), TabId::new(1)],
        "a tab dropped past the tabs of a window's bar joins them at the end"
    );
    assert_eq!(
        state.window_rect(window),
        Some(rect),
        "dropping a tab on a window's bar leaves the window where it is"
    );
    assert!(
        state.entries(state.leaves(state.main())[0]).is_empty(),
        "the pane it came from is left with nothing in it"
    );
}
