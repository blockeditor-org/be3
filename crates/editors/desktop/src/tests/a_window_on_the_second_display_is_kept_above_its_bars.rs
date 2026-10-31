use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_window_on_the_second_display_is_kept_above_its_bars() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(960.0, 640.0));
    let right = Rect::from_min_size(pos2(960.0, 240.0), vec2(640.0, 480.0));
    let mut fixture = Fixture::new();
    fixture.on_two_displays((left, 1.0), (right, 1.0));
    fixture.show_windows(&[host_window(1, true)]);
    fixture.test.click_with(
        "desktop.windows.1",
        block_editor_beui::beui::PointerButton::Secondary,
    );
    fixture.settle();
    fixture.test.click("desktop.windows.1.display.1");
    fixture.settle();
    let grab = fixture.window_grip(1);
    fixture.test.desk_drag(grab, grab + vec2(0.0, 300.0), 8);
    fixture.settle();
    let window = fixture.test.desk_windows()[0];
    let band = fixture.shown_on("dock.bar.band.bottom", right);
    assert!(
        matches!(band[..], [band] if right.contains_rect(window) && window.bottom() <= band.top()),
        "the window dragged into the second display's bars stops above them: {window:?} {band:?}"
    );
    let first = fixture.shown_on("dock.bar.band.bottom", left);
    assert!(
        matches!(first[..], [band] if window.bottom() > band.top()),
        "though it sits lower than the first display's bars, which do not hold it: {window:?} {first:?}"
    );
    fixture
        .test
        .desk_snapshot("a_window_kept_above_the_second_displays_bars", 1);
}
