use be_sdk_beui::beui::vec2;

use super::*;

#[test]
fn a_hidden_window_shown_again_returns_where_it_was_moved() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.show_windows(&[host_window(1, true)]);
    let opened = fixture.window_rect(1).expect("a shown window is drawn");

    let grip = fixture.test.rect_of("dock.window.grip.window.1").center();
    fixture.test.drag(grip, grip + vec2(120.0, 60.0));
    fixture.settle();
    let moved = fixture.window_rect(1).expect("the moved window is drawn");
    assert_ne!(moved.min, opened.min, "dragging the grip moves the window");

    fixture.show_windows(&[HostWindow {
        shown: false,
        ..host_window(1, false)
    }]);
    assert!(fixture.window_rect(1).is_none(), "the host hid the window");
    fixture.show_windows(&[host_window(1, true)]);
    assert_eq!(
        fixture.window_rect(1),
        Some(moved),
        "shown again, the window returns where it was moved, not where it first opened"
    );

    fixture.show_windows(&[]);
    fixture.show_windows(&[host_window(1, true)]);
    assert_eq!(
        fixture.window_rect(1),
        Some(opened),
        "a window that closed and comes back under the same id opens afresh"
    );
}
