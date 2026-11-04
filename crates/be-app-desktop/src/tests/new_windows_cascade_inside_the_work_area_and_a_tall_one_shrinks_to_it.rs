use be_sdk_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn new_windows_cascade_inside_the_work_area_and_a_tall_one_shrinks_to_it() {
    let mut fixture = Fixture::new();
    fixture.test.show_on_displays(&[(
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 720.0)),
        1.0,
    )]);
    fixture.settle();
    let mut windows = vec![TestWindow {
        size: Vec2::new(800.0, 647.0),
        ..host_window(1, false)
    }];
    windows.extend((2..=7).map(|id| TestWindow {
        size: Vec2::new(560.0, 360.0),
        ..host_window(id, id == 7)
    }));
    fixture.show_windows(&windows);
    fixture.settle();
    let band = fixture
        .test
        .desk_rects_of("dock.bar.band.bottom")
        .first()
        .copied()
        .expect("the desktop has a band at its foot");
    let shown = fixture.test.desk_windows();
    assert_eq!(shown.len(), 7, "every window is open: {shown:?}");
    for window in &shown {
        assert!(
            window.top() >= 0.0 && window.bottom() <= band.top() + 0.5,
            "each window opens above the Windows bar's band: {window:?} {band:?}"
        );
    }
    let mut corners: Vec<(i32, i32)> = shown
        .iter()
        .filter(|window| window.width() < 700.0)
        .map(|window| (window.min.x as i32, window.min.y as i32))
        .collect();
    corners.sort();
    corners.dedup();
    assert!(
        corners.len() == 6,
        "the windows still cascade within the room: {shown:?}"
    );
    fixture
        .test
        .desk_snapshot("new_windows_cascade_inside_the_work_area", 0);
}
