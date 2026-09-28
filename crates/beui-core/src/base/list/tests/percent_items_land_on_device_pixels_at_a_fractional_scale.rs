use super::*;

#[test]
fn percent_items_land_on_device_pixels_at_a_fractional_scale() {
    let grid = PixelGrid::new(2.0);
    let sizes = [
        ItemSize::Percent(100.0),
        ItemSize::Percent(100.0),
        ItemSize::Percent(100.0),
    ];
    let intrinsic_lengths = [0.0, 0.0, 0.0];

    let lengths = distribute_main_axis(grid, 10.0, 0.0, &sizes, &intrinsic_lengths);

    for length in &lengths {
        assert_eq!(*length, grid.snap(*length));
    }
    assert_eq!(lengths.iter().sum::<f32>(), 10.0);
}
