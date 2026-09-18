use super::*;

#[test]
fn a_fractional_scale_snaps_to_whole_device_pixels() {
    let grid = PixelGrid::new(1.5);

    assert_eq!(grid.snap(10.0), 10.0);
    assert_eq!(grid.snap(1.0), 2.0 / 1.5);
    assert_eq!(grid.snap(0.1), 0.0);
    assert_eq!(grid.snap_up(0.1), 1.0 / 1.5);
}
