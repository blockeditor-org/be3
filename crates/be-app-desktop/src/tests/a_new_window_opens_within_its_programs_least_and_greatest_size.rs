use be_sdk_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_new_window_opens_within_its_programs_least_and_greatest_size() {
    let mut fixture = Fixture::new();
    let display = Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 720.0));
    fixture.test.show_on_displays(&[(display, 1.0)]);
    fixture.settle();
    let small = HostWindow {
        size: Vec2::new(200.0, 150.0),
        min_size: Vec2::new(480.0, 320.0),
        ..host_window(2, false)
    };
    let capped = HostWindow {
        size: Vec2::new(900.0, 600.0),
        max_size: Vec2::new(400.0, 0.0),
        ..host_window(3, true)
    };
    let huge = HostWindow {
        size: Vec2::new(1337.0, 763.0),
        min_size: Vec2::new(1337.0, 763.0),
        ..host_window(1, false)
    };
    fixture.show_windows(&[huge, small, capped]);
    fixture.settle();
    let band = fixture
        .test
        .desk_rects_of("dock.bar.band.bottom")
        .first()
        .copied()
        .expect("the desktop has a band at its foot");
    let mut shown = fixture.test.desk_windows();
    assert_eq!(shown.len(), 3, "every window is open: {shown:?}");
    shown.sort_by(|left, right| left.width().total_cmp(&right.width()));
    let [capped, small, huge] = [shown[0], shown[1], shown[2]];
    assert!(
        capped.width() >= 400.0 && capped.width() < 460.0 && capped.height() >= 600.0,
        "a window opens at most at its program's greatest size: {shown:?}"
    );
    assert!(
        small.width() >= 480.0 && small.height() >= 320.0,
        "a window opens at least at its program's least size: {shown:?}"
    );
    assert!(
        huge.min.x >= display.min.x
            && huge.max.x <= display.max.x
            && huge.min.y >= display.min.y
            && huge.max.y <= band.top() + 0.5,
        "a window whose least size is more than the work area opens inside it: {huge:?} {band:?}"
    );
    assert!(
        huge.min.x <= small.min.x && huge.min.y <= small.min.y,
        "and at the work area's origin, before every cascaded window: {shown:?}"
    );
    fixture.test.desk_snapshot(
        "a_new_window_opens_within_its_programs_least_and_greatest_size",
        0,
    );
}
