use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_fullscreen_windows_tab_on_another_screen_offers_to_leave_full_screen() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 600.0));
    let right = Rect::from_min_size(pos2(500.0, 0.0), vec2(300.0, 600.0));
    let mut fixture = Fixture::with_windows(&[1]);
    fixture
        .test
        .set_monitors(vec![("left", left), ("right", right)]);
    fixture.settle();
    let fullscreen = HostWindow {
        fullscreen: Some(right),
        ..host_window(1, true)
    };
    fixture
        .test
        .set_host_value::<HostWindows>(&vec![fullscreen]);
    fixture.settle();
    fixture.test.clear_calls();
    assert!(
        right.contains_rect(fixture.test.rect_of("dock.fullscreen")),
        "the window fills the screen it is fullscreen on"
    );
    assert!(
        left.contains_rect(fixture.test.rect_of("dock.away.leave")),
        "its tab, on the other screen, says it is full screen and offers the way back"
    );
    fixture
        .test
        .snapshot("a_fullscreen_windows_tab_on_another_screen");

    fixture.test.click("dock.away.leave");
    fixture.settle();
    assert_eq!(
        fixture.test.actions::<WindowAction>(),
        [WindowAction::Fullscreen {
            window: HostWindowId(1),
            fullscreen: false
        }],
        "the button asks the host to take the window out of fullscreen"
    );
    assert!(!fixture.test.shown("dock.fullscreen"));
    assert!(!fixture.test.shown("dock.away"));
}
