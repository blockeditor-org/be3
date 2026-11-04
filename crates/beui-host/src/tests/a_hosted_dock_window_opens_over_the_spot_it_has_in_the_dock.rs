use super::*;
use crate::{WindowId, WindowPlacement, WindowRequest};

#[test]
fn a_hosted_dock_window_opens_over_the_spot_it_has_in_the_dock() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let window = window_showing(&harness, dock, "Tab 3");
    let placement = harness
        .take_window_requests()
        .into_iter()
        .find_map(|request| match request {
            WindowRequest::Open {
                window: opened,
                placement,
                ..
            } if opened == window => Some(placement),
            _ => None,
        });
    assert_eq!(
        placement,
        Some(WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(52.0, 52.0),
        }),
        "the window opens where the dock would draw it in the main window, grips and all"
    );
}
