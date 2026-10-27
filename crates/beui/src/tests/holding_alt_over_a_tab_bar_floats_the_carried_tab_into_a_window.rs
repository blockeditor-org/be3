use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn holding_alt_over_a_tab_bar_floats_the_carried_tab_into_a_window() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let from = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let over = harness.center(dock_tab(harness.document(), dock, "Tab 1"));

    harness.press_at(from);
    harness.frame(vec![Input::PointerMoved(over)]);
    harness.frames_to_settle();
    harness.frame(vec![Input::Modifiers(Modifiers::ALT)]);
    harness.frames_to_settle();
    harness.frame(vec![Input::PointerButton {
        pos: over,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    harness.frame(vec![Input::Modifiers(Modifiers::NONE)]);
    harness.frames_to_settle();

    let state = harness.document().api::<DockApi>(dock).state();
    let windows = state.windows();
    assert_eq!(
        windows.len(),
        1,
        "a tab let go over its bar with alt held floats into a window rather than reordering the bar"
    );
    assert_eq!(state.surface_tabs(windows[0]), vec![TabId::new(3)]);
    assert_eq!(
        state.surface_tabs(state.main()),
        vec![TabId::new(1), TabId::new(2)],
        "and leaves the bar it was carried along"
    );
}
