use super::*;
use crate::WindowRequest;
use crate::dock::unstyled::DockApi;

#[test]
fn dragging_a_hosted_dock_windows_bar_asks_its_host_to_move_it() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(None);
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let window = window_showing(&harness, dock, "Tab 3");
    assert!(
        !window.is_main(),
        "the dock's window is a window of its own"
    );
    let state = harness.document().api::<DockApi>(dock).state();
    let surface = state.windows()[0];
    let before = state.window_rect(surface);
    let tab = harness.rect(dock_tab(harness.document(), dock, "Tab 3"));
    let bar = pos2(tab.right() + 40.0, tab.center().y);
    harness.take_window_requests();

    press_in(&mut harness, window, bar, true);
    harness.frame_in(window, vec![Input::PointerMoved(bar + vec2(30.0, 20.0))]);
    press_in(&mut harness, window, bar + vec2(30.0, 20.0), false);

    assert!(
        harness
            .take_window_requests()
            .contains(&WindowRequest::BeginMove(window)),
        "a press on the bar hands the move to the host"
    );
    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.window_rect(surface),
        before,
        "the dock leaves moving the window to its host"
    );
}
