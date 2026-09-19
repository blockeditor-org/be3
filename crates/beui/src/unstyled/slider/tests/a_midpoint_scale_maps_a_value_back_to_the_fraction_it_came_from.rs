use super::*;

#[test]
fn a_midpoint_scale_maps_a_value_back_to_the_fraction_it_came_from() {
    for scale in [SliderScale::Midpoint(12.0), SliderScale::Midpoint(100.0)] {
        for fraction in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
            let value = scale.value_at(fraction, 0.0, 120.0);
            let back = scale.fraction_of(value, 0.0, 120.0);
            assert!(
                (back - fraction).abs() < 0.001,
                "{scale:?} turned {fraction} into {value} and back into {back}"
            );
        }
    }
}
