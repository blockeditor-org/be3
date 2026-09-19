use super::*;

#[test]
fn a_midpoint_scale_puts_its_midpoint_at_the_centre_of_the_track() {
    let scale = SliderScale::Midpoint(12.0);

    assert!((scale.value_at(0.0, 0.0, 120.0)).abs() < 0.001);
    assert!((scale.value_at(0.25, 0.0, 120.0) - 3.0).abs() < 0.001);
    assert!((scale.value_at(0.5, 0.0, 120.0) - 12.0).abs() < 0.001);
    assert!((scale.value_at(0.75, 0.0, 120.0) - 39.0).abs() < 0.001);
    assert!((scale.value_at(1.0, 0.0, 120.0) - 120.0).abs() < 0.001);
    assert!((scale.fraction_of(12.0, 0.0, 120.0) - 0.5).abs() < 0.001);
}
