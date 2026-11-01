use super::*;

#[test]
fn screen_and_world_points_follow_the_host_view() {
    let rect = Rect::from_min_size(Pos2::new(20.0, 40.0), Vec2::new(800.0, 600.0));
    let view = CanvasView::new(Pos2::new(-30.0, 15.0), 2.0);
    let world = Vec2::new(400.0, 300.0);
    let grid = grid_view(Some(view), Some(world), rect);

    assert_eq!(grid.scale, CELL * 2.0);
    let origin = grid.to_screen(Pos2::ZERO);
    let middle = view.to_screen(Pos2::new(world.x * 0.5, world.y * 0.5));
    assert!((origin.x - middle.x).abs() < 0.001);
    assert!((origin.y - middle.y).abs() < 0.001);
}
