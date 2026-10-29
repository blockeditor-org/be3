use super::*;
use crate::dock::unstyled::{DockApi, TabId};
use crate::{WindowId, WindowPlacement, WindowRequest};

#[test]
fn a_dock_tab_dropped_where_no_window_is_opens_a_window_at_the_pointer() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    let before = harness.document().api::<DockApi>(dock).state().windows();
    harness.take_window_requests();

    press_in(&mut harness, WindowId::MAIN, tab, true);
    harness.frame(vec![Input::PointerMoved(tab + vec2(30.0, 30.0))]);
    harness.frame(vec![Input::PointerLeft]);
    let nowhere = pos2(-60.0, -40.0);
    harness.frame(vec![Input::PointerMoved(nowhere)]);
    press_in(&mut harness, WindowId::MAIN, nowhere, false);
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
    let opened: Vec<_> = state
        .windows()
        .into_iter()
        .filter(|surface| !before.contains(surface))
        .collect();
    let [surface] = opened.as_slice() else {
        panic!("the drop opens one window, not {opened:?}");
    };
    assert_eq!(
        state.surface_tabs(*surface),
        vec![TabId::new(1)],
        "holding the tab dropped"
    );
    let window = window_showing(&harness, dock, "Tab 1");
    assert!(
        harness
            .take_window_requests()
            .iter()
            .any(|request| matches!(
                request,
                WindowRequest::Open { window: opened, placement: WindowPlacement::AtPointer, .. }
                    if *opened == window
            )),
        "the new window opens at the pointer"
    );
}
