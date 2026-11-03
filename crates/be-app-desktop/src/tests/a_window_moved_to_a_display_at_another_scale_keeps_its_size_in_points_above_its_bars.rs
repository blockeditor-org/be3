use be_sdk_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_window_moved_to_a_display_at_another_scale_keeps_its_size_in_points_above_its_bars() {
    let laptop = Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0));
    let monitor = Rect::from_min_size(pos2(1280.0, -280.0), vec2(1920.0, 1080.0));
    let mut fixture = Fixture::new();
    fixture.on_two_displays((laptop, 2.0), (monitor, 1.0));
    fixture.show_windows(&[host_window(1, true)]);
    let was = fixture.test.desk_windows()[0];
    assert!(laptop.contains_rect(was));

    assert!(fixture.test.host_key(
        Modifiers {
            shift: true,
            ..Modifiers::LOGO
        },
        Key::ArrowRight
    ));
    fixture.settle();
    let moved = fixture.test.desk_windows()[0];
    assert!(
        monitor.contains_rect(moved),
        "the window moves onto the display at 1x: {moved:?}"
    );
    assert_eq!(
        moved.size(),
        was.size(),
        "at the size in points it had on the display at 2x"
    );
    let band = fixture.shown_on("dock.bar.band.bottom", monitor);
    assert!(
        matches!(band[..], [band] if moved.bottom() <= band.top()),
        "above that display's own bars: {moved:?} {band:?}"
    );

    assert!(fixture.test.host_key(
        Modifiers {
            shift: true,
            ..Modifiers::LOGO
        },
        Key::ArrowLeft
    ));
    fixture.settle();
    let back = fixture.test.desk_windows()[0];
    let band = fixture.shown_on("dock.bar.band.bottom", laptop);
    assert!(
        laptop.contains_rect(back) && back.size() == was.size(),
        "and back on the display at 2x as it was: {back:?}"
    );
    assert!(
        matches!(band[..], [band] if back.bottom() <= band.top()),
        "above its bars: {back:?} {band:?}"
    );
    fixture
        .test
        .desk_snapshot("a_window_back_on_the_display_at_2x", 0);
}
