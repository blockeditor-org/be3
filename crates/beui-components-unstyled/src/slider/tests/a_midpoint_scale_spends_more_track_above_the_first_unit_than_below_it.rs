use super::*;

#[test]
fn a_midpoint_scale_spends_more_track_above_the_first_unit_than_below_it() {
    let scale = SliderScale::Midpoint(12.0);

    let below_one = scale.fraction_of(1.0, 0.0, 120.0);
    let up_to_the_midpoint = scale.fraction_of(12.0, 0.0, 120.0) - below_one;

    assert!(
        below_one < 0.15,
        "the first pixel took {below_one} of the track"
    );
    assert!(
        up_to_the_midpoint > below_one * 3.0,
        "the track below one read {below_one} against {up_to_the_midpoint} above it"
    );
}
