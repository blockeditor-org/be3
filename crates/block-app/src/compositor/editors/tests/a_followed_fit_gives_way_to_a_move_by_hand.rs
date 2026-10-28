use super::*;
use beui::pos2;

#[test]
fn a_followed_fit_gives_way_to_a_move_by_hand() {
    let rect = Rect::from_min_size(pos2(100.0, 50.0), vec2(400.0, 300.0));
    let content = vec2(800.0, 300.0);
    let world = Some(FitArea::World);
    let mut viewport = Viewport::default();

    follow(&mut viewport, world, rect, content, MAX_ZOOM);
    assert_eq!((viewport.zoom, viewport.pan), (0.5, Vec2::ZERO));
    let fitted = viewport;
    follow(&mut viewport, world, rect, content, MAX_ZOOM);
    assert_eq!(viewport, fitted, "following again lands on the same view");

    follow(
        &mut viewport,
        Some(FitArea::Centre),
        rect,
        content,
        MAX_ZOOM,
    );
    assert_eq!((viewport.zoom, viewport.pan), (1.0, Vec2::ZERO));

    settle(&mut viewport, Command::Pan(vec2(0.0, 5.0)), rect, MAX_ZOOM);
    assert!(
        viewport.moved,
        "a pan the person made moves the view by hand"
    );
    let moved = viewport;
    follow(&mut viewport, world, rect, content, MAX_ZOOM);
    assert_eq!(
        viewport, moved,
        "a fit kept while the view is moved by hand leaves it where the person put it"
    );

    viewport.moved = false;
    follow(&mut viewport, world, rect, content, MAX_ZOOM);
    assert_eq!(
        viewport, fitted,
        "a Fit the person pressed follows the content again"
    );
}
