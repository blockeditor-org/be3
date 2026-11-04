use super::*;

#[test]
fn growing_items_get_nothing_when_no_space_remains() {
    let sizes = [ItemSize::Intrinsic, ItemSize::Grow(1.0)];
    let intrinsic_lengths = [150.0, 0.0];

    let lengths =
        distribute_main_axis(PixelGrid::default(), 100.0, 0.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![150.0, 0.0]);
}
