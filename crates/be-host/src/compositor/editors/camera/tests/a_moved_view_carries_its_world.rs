use super::*;
use beui::pos2;

#[test]
fn a_moved_view_carries_its_world() {
    let area = Rect::from_min_size(pos2(100.0, 50.0), vec2(400.0, 300.0));
    let content = vec2(800.0, 300.0);
    let by = vec2(120.0, -30.0);
    let moved_area = area.translate(by);
    let cases = [
        ("an unmoved camera", Viewport::default(), None),
        (
            "an unmoved camera keeping its world fitted",
            Viewport::default(),
            Some(FitArea::World),
        ),
        (
            "a moved camera",
            Viewport {
                zoom: 2.0,
                pan: vec2(30.0, -10.0),
                moved: true,
            },
            Some(FitArea::World),
        ),
    ];
    for (camera, mut viewport, fitted) in cases {
        follow(&mut viewport, fitted, area, content, MAX_ZOOM);
        let before = viewport.shown(area, content);
        follow(&mut viewport, fitted, moved_area, content, MAX_ZOOM);
        let after = viewport.shown(moved_area, content);
        assert_eq!(
            after,
            before.translate(by),
            "{camera} moves its world with its region"
        );
    }
}
