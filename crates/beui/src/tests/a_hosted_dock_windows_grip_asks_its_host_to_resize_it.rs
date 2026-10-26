use super::*;
use crate::dock::unstyled::DockApi;
use crate::{WindowEdge, WindowRequest};

#[test]
fn a_hosted_dock_windows_grip_asks_its_host_to_resize_it() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let window = window_showing(&harness, dock, "Tab 3");
    let size = harness
        .windows()
        .window(window)
        .expect("the host opened the window")
        .size;
    let state = harness.document().api::<DockApi>(dock).state();
    let surface = state.windows()[0];
    let before = state.window_rect(surface);
    harness.take_window_requests();

    let corner = pos2(size.x - 3.0, size.y - 3.0);
    press_in(&mut harness, window, corner, true);
    harness.frame_in(window, vec![Input::PointerMoved(corner + vec2(40.0, 30.0))]);
    press_in(&mut harness, window, corner + vec2(40.0, 30.0), false);

    assert!(
        harness
            .take_window_requests()
            .contains(&WindowRequest::BeginResize {
                window,
                edge: WindowEdge::BottomRight
            }),
        "a press on the corner grip hands the resize to the host"
    );
    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.window_rect(surface),
        before,
        "the dock waits for the host to report the new size"
    );
}
