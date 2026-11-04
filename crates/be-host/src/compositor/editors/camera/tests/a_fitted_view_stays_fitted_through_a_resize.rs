use super::*;
use beui::pos2;

#[test]
fn a_fitted_view_stays_fitted_through_a_resize() {
    let content = vec2(800.0, 300.0);
    let mut viewport = Viewport::default();
    for area in [
        Rect::from_min_size(pos2(100.0, 50.0), vec2(400.0, 300.0)),
        Rect::from_min_size(pos2(100.0, 50.0), vec2(200.0, 300.0)),
        Rect::from_min_size(pos2(260.0, 50.0), vec2(1600.0, 150.0)),
    ] {
        follow(&mut viewport, Some(FitArea::World), area, content, MAX_ZOOM);
        let shown = viewport.shown(area, content);
        assert_eq!(
            shown.center(),
            area.center(),
            "the world is centred in {area:?}"
        );
        let fills = (shown.width() - area.width()).abs() < 0.01
            || (shown.height() - area.height()).abs() < 0.01
            || viewport.zoom == 1.0;
        assert!(
            fills && area.contains_rect(shown.shrink(0.01)),
            "the world, at {shown:?}, fits {area:?}"
        );
    }
}
