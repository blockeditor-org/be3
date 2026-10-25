use super::*;
use crate::unstyled::{DockApi, OwnedDockingLayout};
use crate::{HostPlacement, WindowPlacement, WindowReport, WindowRequest};

#[test]
fn a_hosted_dock_window_reopens_where_its_host_placed_it_after_the_layout_is_restored() {
    let _root = crate::reactive::RootScope::new();
    let layout = OwnedDockingLayout::<u64>::new();
    let (document, dock) = windowed_dock_in(Some(layout.layout()), None);
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let window = window_showing(&harness, dock, "Tab 3");
    let surface = harness.document().api::<DockApi>(dock).state().windows()[0];
    let pose = HostPlacement("space 1 1.5 -2 0 0 0 1 0.002 0.5".to_owned());
    harness.report_window(WindowReport::Placed {
        window,
        placement: pose.clone(),
    });
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .window_placement(surface),
        Some(pose.clone()),
        "the dock keeps the host's placement in its state"
    );
    let snapshot = layout.snapshot();

    let restored = OwnedDockingLayout::<u64>::new();
    restored.restore(snapshot);
    let (document, _) = windowed_dock_in(Some(restored.layout()), None);
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let placements: Vec<WindowPlacement> = harness
        .take_window_requests()
        .into_iter()
        .filter_map(|request| match request {
            WindowRequest::Open { placement, .. } => Some(placement),
            _ => None,
        })
        .collect();
    assert!(
        placements.contains(&WindowPlacement::Host(pose)),
        "the window opens again where its host placed it: {placements:?}"
    );
}
