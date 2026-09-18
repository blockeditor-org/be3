use super::*;

#[test]
fn snapping_up_tolerates_floating_point_drift() {
    let grid = PixelGrid::new(1.0);

    assert_eq!(grid.snap_up(30.000004), 30.0);
    assert_eq!(grid.snap_up(30.01), 31.0);
    assert_eq!(grid.snap_up(29.999996), 30.0);
}
