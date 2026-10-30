use block_editor_beui::beui::{Pos2, Vec2};

use super::*;

#[test]
fn a_fullscreen_window_covers_the_bars() {
    let mut fixture = Fixture::with_windows(&[1]);
    let band = fixture.test.rect_of("dock.bar.band.bottom");
    let screen = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
    fixture.show_windows(&[HostWindow {
        fullscreen: Some(screen),
        ..host_window(1, true)
    }]);
    let shown = fixture.window_rect(1).expect("the window is placed");
    assert!(
        shown.contains_rect(band),
        "the fullscreen window covers the bars too: {shown:?} {band:?}"
    );
    fixture.test.snapshot("a_fullscreen_window_over_the_bars");
}
