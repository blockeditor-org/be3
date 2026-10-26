use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn each_display_shows_its_own_bar_at_its_own_foot() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(480.0, 600.0));
    let right = Rect::from_min_size(pos2(480.0, 240.0), vec2(320.0, 360.0));
    let mut fixture = Fixture::new();
    fixture.report_screens(vec![monitor("left", left, 1.0), monitor("right", right, 2.0)]);
    let bars = fixture.test.rects_of("desktop.bar");
    assert_eq!(bars.len(), 2, "one bar per display: {bars:?}");
    for (display, bar) in [left, right].iter().zip(&bars) {
        assert!(
            display.contains_rect(*bar),
            "each bar lies on its own display: {bar:?} on {display:?}"
        );
        assert_eq!(bar.width(), display.width(), "and spans it");
        assert_eq!(bar.bottom(), display.bottom(), "along its foot");
    }
    fixture.test.snapshot("each_display_with_its_own_bar");
}
