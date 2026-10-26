use super::*;
use crate::dock::unstyled::DockApi;
use crate::{WindowEdge, WindowRequest};

#[test]
fn a_secondary_drag_with_the_dock_drag_modifier_hands_a_hosted_windows_resize_to_its_host() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = windowed_dock(Some(Modifiers::LOGO));
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

    let from = pos2(size.x - 30.0, size.y - 30.0);
    let to = from + vec2(-60.0, -30.0);
    let secondary = |pos, pressed| Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed,
    };
    harness.frame_in(
        window,
        vec![Input::PointerMoved(from), Input::Modifiers(Modifiers::LOGO)],
    );
    harness.frame_in(window, vec![secondary(from, true)]);
    harness.frame_in(window, vec![Input::PointerMoved(to)]);
    harness.frame_in(window, vec![secondary(to, false)]);
    harness.frame_in(window, vec![Input::Modifiers(Modifiers::NONE)]);

    assert!(
        harness
            .take_window_requests()
            .contains(&WindowRequest::BeginResize(window, WindowEdge::BottomRight)),
        "the drag hands the nearest corner to the host"
    );
    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .window_rect(surface),
        before,
        "the dock waits for the host to report the new size"
    );
}
