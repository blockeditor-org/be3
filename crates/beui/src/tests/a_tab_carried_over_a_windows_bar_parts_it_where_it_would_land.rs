use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn a_tab_carried_over_a_windows_bar_parts_it_where_it_would_land() {
    let _root = crate::reactive::RootScope::new();
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
    harness.frames_to_settle();
    let window = harness.document().api::<DockApi>(dock).state().windows()[0];
    let floated = dock_tab_extent(&harness, dock, "Tab 2", 2);

    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    let over = pos2(floated.left() - 10.0, floated.center().y);
    harness.press_at(tab);
    harness.frame(vec![Input::PointerMoved(over)]);
    harness.frames_to_settle();

    let parted = dock_tab_extent(&harness, dock, "Tab 2", 2);
    assert!(
        parted.left() > floated.left() + floated.width() / 2.0,
        "the window's tab glides aside to open a gap for the tab carried in front of it: {} became {}",
        floated.left(),
        parted.left()
    );
    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.windows(),
        vec![window],
        "hovering a window's bar with a tab does not open another window"
    );
    assert_eq!(state.surface_tabs(window), vec![TabId::new(2)]);

    harness.release_at(over);
    harness.frames_to_settle();

    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.surface_tabs(window),
        vec![TabId::new(1), TabId::new(2)],
        "the tab dropped in the gap joins the window before the tab it parted"
    );
}
