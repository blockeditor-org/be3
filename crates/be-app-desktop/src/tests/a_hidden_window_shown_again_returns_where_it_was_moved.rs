use be_sdk_beui::beui::vec2;

use super::*;

#[test]
fn a_hidden_window_shown_again_returns_where_it_was_moved() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.show_windows(&[host_window(1, true)]);
    let opened = fixture.window_rect(1).expect("a shown window is drawn");

    let grip = fixture
        .test
        .rect_of(&format!("dock.window.grip.window.{}", fixture.tab_of(1)))
        .center();
    fixture.test.drag(grip, grip + vec2(120.0, 60.0));
    fixture.settle();
    let moved = fixture.window_rect(1).expect("the moved window is drawn");
    assert_ne!(moved.min, opened.min, "dragging the grip moves the window");

    fixture.test.click_with(
        "desktop.windows.1",
        be_sdk_beui::beui::PointerButton::Secondary,
    );
    fixture.settle();
    fixture.test.click("desktop.windows.1.visibility");
    fixture.settle();
    assert!(fixture.window_rect(1).is_none(), "the window is hidden");
    fixture.test.click("desktop.windows.1");
    fixture.settle();
    assert_eq!(
        fixture.window_rect(1),
        Some(moved),
        "shown again, the window returns where it was moved, not where it first opened"
    );

    fixture.show_windows(&[]);
    fixture.show_windows(&[host_window(1, true)]);
    let reopened = fixture.window_rect(1).expect("the window is drawn");
    assert_eq!(
        reopened.size(),
        opened.size(),
        "a window that closed and comes back under the same id opens afresh"
    );
    assert_ne!(
        reopened.min, moved.min,
        "not where the closed one was moved"
    );
}
