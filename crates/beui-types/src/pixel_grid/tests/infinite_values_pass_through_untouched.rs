use super::*;

#[test]
fn infinite_values_pass_through_untouched() {
    let grid = PixelGrid::new(2.0);

    assert_eq!(grid.snap(f32::INFINITY), f32::INFINITY);
    assert_eq!(grid.snap_up(f32::INFINITY), f32::INFINITY);
    assert_eq!(grid.snap_size(Vec2::INFINITY).x, f32::INFINITY);
}
