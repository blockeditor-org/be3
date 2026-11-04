use super::*;

#[test]
fn growing_items_share_by_any_scale_of_weights() {
    let sizes = [ItemSize::Grow(1.0), ItemSize::Grow(3.0)];
    let intrinsic_lengths = [0.0, 0.0];

    let lengths =
        distribute_main_axis(PixelGrid::default(), 100.0, 0.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![25.0, 75.0]);
}
