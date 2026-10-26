use super::*;
use crate::dock::unstyled::DockApi;
use crate::{HostPlacement, WindowPlacement, WindowReport, WindowRequest};

#[test]
fn a_hosted_dock_window_keeps_the_size_and_place_its_host_reports_and_reopens_there() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let window = window_showing(&harness, dock, "Tab 3");
    let state = harness.document().api::<DockApi>(dock).state();
    let surface = state.windows()[0];
    let origin = state.window_rect(surface).expect("the window is open").min;

    let pose = HostPlacement("over by the door".to_owned());
    harness.report_window(WindowReport::Resized {
        window,
        size: vec2(500.0, 400.0),
    });
    harness.report_window(WindowReport::Placed {
        window,
        placement: pose.clone(),
    });
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.window_rect(surface),
        Some(Rect::from_min_size(origin, vec2(484.0, 384.0))),
        "the dock keeps the size the host gave the window, less its grips"
    );

    harness.resize(vec2(600.0, WIDE_VIEWPORT.y));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness.windows().window(window).is_none(),
        "a stacked dock shows no windows"
    );
    harness.take_window_requests();
    harness.resize(WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let reopened = harness
        .take_window_requests()
        .into_iter()
        .find_map(|request| match request {
            WindowRequest::Open {
                placement, size, ..
            } if placement == WindowPlacement::Host(pose.clone()) => Some(size),
            _ => None,
        });
    assert_eq!(
        reopened,
        Some(vec2(500.0, 400.0)),
        "the window opens again where its host last put it, at the size it had"
    );
}
