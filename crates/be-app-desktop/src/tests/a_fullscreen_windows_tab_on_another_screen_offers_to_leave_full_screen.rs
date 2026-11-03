use be_sdk_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_fullscreen_windows_tab_on_another_screen_offers_to_leave_full_screen() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 600.0));
    let right = Rect::from_min_size(pos2(500.0, 0.0), vec2(300.0, 600.0));
    let mut fixture = Fixture::with_windows(&[1]);
    fixture.report_screens(vec![
        monitor("left", left, 1.0),
        monitor("right", right, 1.0),
    ]);
    fixture.test.click_with("desktop.windows.1", be_sdk_beui::beui::PointerButton::Secondary);
    fixture.settle();
    fixture.test.click("desktop.windows.1.display.1");
    fixture.settle();
    let fullscreen = TestWindow {
        fullscreen: Some(right),
        ..host_window(1, true)
    };
    fixture.show_windows(&vec![fullscreen]);
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
    assert!(
        fixture.fullscreen_windows().is_empty(),
        "the button takes the window out of full screen, which the host tells its program"
    );
    assert!(!fixture.test.shown("dock.fullscreen"));
    assert!(!fixture.test.shown("dock.away"));
}
