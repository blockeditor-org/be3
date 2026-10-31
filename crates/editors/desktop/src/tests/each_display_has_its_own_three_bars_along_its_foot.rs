use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn each_display_has_its_own_three_bars_along_its_foot() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(960.0, 640.0));
    let right = Rect::from_min_size(pos2(960.0, 240.0), vec2(640.0, 400.0));
    let mut fixture = Fixture::new();
    fixture.on_two_displays((left, 1.0), (right, 2.0));
    let bands = fixture.test.desk_rects_of("dock.bar.band.bottom");
    assert_eq!(bands.len(), 2, "one band per display: {bands:?}");
    for display in [left, right] {
        let band = bands
            .iter()
            .copied()
            .find(|band| display.contains_rect(*band))
            .unwrap_or_else(|| panic!("a band lies on {display:?}: {bands:?}"));
        assert_eq!(band.width(), display.width(), "and spans it");
        assert_eq!(band.bottom(), display.bottom(), "along its foot");
        for bar in [
            "desktop.bar.apps",
            "desktop.bar.windows",
            "desktop.bar.status",
        ] {
            assert!(
                matches!(fixture.shown_on(bar, display)[..], [shown] if shown.intersects(band)),
                "{bar} sits in the band of {display:?}: {:?}",
                fixture.test.desk_rects_of(bar)
            );
        }
    }
    fixture
        .test
        .desk_snapshot("each_display_with_its_own_bars_left", 0);
    fixture
        .test
        .desk_snapshot("each_display_with_its_own_bars_right", 1);
}
