use super::*;

#[test]
fn screen_and_world_points_follow_the_host_view() {
    let rect = Rect::from_min_size(Pos2::new(20.0, 40.0), Vec2::new(800.0, 600.0));
    let view = CanvasView::new(Pos2::new(-30.0, 15.0), 2.0);
    let world = Vec2::new(400.0, 300.0);
    let camera = Camera::from_view(Some(view), Some(world), rect);

    assert_eq!(camera.zoom, CELL * 2.0);
    let origin = camera.world_to_screen([0.0, 0.0], rect);
    let middle = view.to_screen(Pos2::new(world.x * 0.5, world.y * 0.5));
    assert!((origin.x - middle.x).abs() < 0.001);
    assert!((origin.y - middle.y).abs() < 0.001);

    let cursor = Pos2::new(187.0, 249.0);
    let back = camera.world_to_screen(camera.screen_to_world(cursor, rect), rect);
    assert!((back.x - cursor.x).abs() < 0.001);
    assert!((back.y - cursor.y).abs() < 0.001);
}
