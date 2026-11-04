use super::*;
use beui::pos2;

#[test]
fn a_moved_camera_keeps_its_centre_through_a_resize() {
    let area = Rect::from_min_size(pos2(100.0, 50.0), vec2(400.0, 300.0));
    let resized = Rect::from_min_size(pos2(100.0, 50.0), vec2(700.0, 180.0));
    let content = vec2(800.0, 300.0);
    let mut viewport = Viewport {
        zoom: 2.0,
        pan: vec2(40.0, 20.0),
        moved: true,
    };
    let centre = |viewport: Viewport, area: Rect| {
        let shown = viewport.shown(area, content);
        (area.center() - shown.min) / viewport.zoom
    };

    let before = centre(viewport, area);
    follow(
        &mut viewport,
        Some(FitArea::World),
        resized,
        content,
        MAX_ZOOM,
    );
    assert_eq!(
        centre(viewport, resized),
        before,
        "the point of the world in the middle of the content stays there"
    );
    assert_eq!(viewport.zoom, 2.0, "the camera keeps its zoom");
}
