use super::*;

#[test]
fn growing_items_split_remaining_space_by_weight() {
    let sizes = [ItemSize::Grow(0.5), ItemSize::Grow(0.5)];
    let intrinsic_lengths = [0.0, 0.0];

    let lengths =
        distribute_main_axis(PixelGrid::default(), 100.0, 0.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![50.0, 50.0]);
}
