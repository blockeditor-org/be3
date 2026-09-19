use super::*;

#[test]
fn a_midpoint_at_the_centre_or_outside_the_range_leaves_the_slider_linear() {
    let midpoints = [0.0, 60.0, 120.0, -5.0, 200.0, f32::NAN];

    for midpoint in midpoints {
        let scale = SliderScale::Midpoint(midpoint);
        let value = scale.value_at(0.25, 0.0, 120.0);
        assert!(
            (value - 30.0).abs() < 0.001,
            "a midpoint of {midpoint} read {value}"
        );
    }
}
