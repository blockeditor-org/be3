use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn each_display_shows_its_own_bar_at_its_own_foot() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(960.0, 640.0));
    let right = Rect::from_min_size(pos2(960.0, 240.0), vec2(640.0, 400.0));
    let mut fixture = Fixture::new();
    fixture.report_screens(vec![
        monitor("left", left, 1.0),
        monitor("right", right, 2.0),
    ]);
    fixture.test.show_on_displays(&[(left, 1.0), (right, 2.0)]);
    fixture.settle();
    let mut bars = fixture.test.desk_rects_of("dock.bar.band.bottom");
    bars.extend(fixture.test.desk_rects_of("desktop.bar"));
    assert_eq!(bars.len(), 2, "one bar per display: {bars:?}");
    for display in [left, right] {
        let bar = bars
            .iter()
            .find(|bar| display.contains_rect(**bar))
            .unwrap_or_else(|| panic!("a bar lies on {display:?}: {bars:?}"));
        assert_eq!(bar.width(), display.width(), "and spans it");
        assert_eq!(bar.bottom(), display.bottom(), "along its foot");
    }
    fixture
        .test
        .desk_snapshot("each_display_with_its_own_bar_left", 0);
    fixture
        .test
        .desk_snapshot("each_display_with_its_own_bar_right", 1);
}
