use super::*;

#[test]
fn a_linear_scale_maps_the_fraction_straight_onto_the_range() {
    let scale = SliderScale::Linear;

    assert!((scale.value_at(0.0, 0.5, 3.0) - 0.5).abs() < 0.001);
    assert!((scale.value_at(0.5, 0.5, 3.0) - 1.75).abs() < 0.001);
    assert!((scale.value_at(1.0, 0.5, 3.0) - 3.0).abs() < 0.001);
    assert!((scale.fraction_of(1.75, 0.5, 3.0) - 0.5).abs() < 0.001);
}
