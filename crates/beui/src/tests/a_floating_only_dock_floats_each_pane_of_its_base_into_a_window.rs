use super::*;
use crate::unstyled::{DockApi, TabId};
use crate::{WindowId, WindowPlacement, WindowRequest};

#[test]
fn a_floating_only_dock_floats_each_pane_of_its_base_into_a_window() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(true);
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
    assert!(
        state
            .leaves(state.main())
            .into_iter()
            .all(|leaf| state.entries(leaf).is_empty()),
        "nothing is tiled in the base"
    );
    assert_eq!(state.windows().len(), 4, "each pane has a window");
    let left = state.find(TabId::new(1)).expect("tab 1 is open").surface;
    let right = state.find(TabId::new(2)).expect("tab 2 is open").surface;
    assert_ne!(left, right, "each pane floats into a window of its own");
    assert_eq!(state.surface_tabs(left), vec![TabId::new(1)]);
    assert_eq!(state.surface_tabs(right), vec![TabId::new(2)]);

    let window = window_showing(&harness, dock, "Tab 1");
    assert!(!window.is_main(), "the floated pane shows in a window");
    assert!(
        harness.take_window_requests().iter().any(|request| matches!(
            request,
            WindowRequest::Open { window: opened, placement: WindowPlacement::Beside(beside), .. }
                if *opened == window && *beside == WindowId::MAIN
        )),
        "a floated pane opens beside the main window"
    );
}
