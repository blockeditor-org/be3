use be_sdk_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_new_window_that_asked_to_be_maximized_opens_filling_its_area() {
    let mut fixture = Fixture::new();
    fixture.test.show_on_displays(&[(
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 720.0)),
        1.0,
    )]);
    fixture.settle();
    let firefox = TestWindow {
        size: Vec2::new(500.0, 200.0),
        maximized: true,
        ..host_window(1, true)
    };
    fixture.show_windows(&[firefox]);
    let shown = fixture.test.desk_windows();
    let [shown] = shown[..] else {
        panic!("the window is open: {shown:?}");
    };
    assert!(
        shown.width() > 1000.0 && shown.height() > 550.0,
        "a window whose program asked to be maximized (Firefox does, then commits GTK's small default) fills the work area rather than opening at that size: {shown:?}"
    );
}
