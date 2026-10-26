use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn a_tab_dragged_from_one_hosted_dock_window_onto_anothers_bar_joins_it() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let from = window_showing(&harness, dock, "Tab 3");
    let to = window_showing(&harness, dock, "Tab 4");
    assert_ne!(from, to, "each dock window is a window of its own");
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let onto = harness.rect(dock_tab(harness.document(), dock, "Tab 4"));
    let bar = pos2(onto.right() + 20.0, onto.center().y);
    let state = harness.document().api::<DockApi>(dock).state();
    let target = state
        .find(TabId::new(4))
        .expect("tab 4 is in the dock")
        .surface;

    press_in(&mut harness, from, tab, true);
    harness.frame_in(from, vec![Input::PointerMoved(tab + vec2(20.0, 20.0))]);
    harness.frame_in(to, vec![Input::PointerMoved(bar - vec2(10.0, 0.0))]);
    harness.frame_in(to, vec![Input::PointerMoved(bar)]);
    press_in(&mut harness, to, bar, false);
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.surface_tabs(target),
        vec![TabId::new(4), TabId::new(3)],
        "the tab joins the bar of the window it was dropped on"
    );
    assert_eq!(
        state.windows().len(),
        1,
        "the window it left, now empty, is gone"
    );
}
