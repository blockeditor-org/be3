use super::*;

#[test]
fn percent_items_split_an_odd_space_into_whole_pixels() {
    let sizes = [ItemSize::Percent(50.0), ItemSize::Percent(50.0)];
    let intrinsic_lengths = [0.0, 0.0];

    let lengths =
        distribute_main_axis(PixelGrid::default(), 101.0, 0.0, &sizes, &intrinsic_lengths);

    assert_eq!(lengths, vec![51.0, 50.0]);
    assert_eq!(lengths.iter().sum::<f32>(), 101.0);
}
