use crate::color::{Color32, Oklch};
use crate::unstyled::WheelPoint;

#[test]
fn every_srgb_color_has_a_place_in_an_oklch_wheels_triangle() {
    for red in (0..=255).step_by(15) {
        for green in (0..=255).step_by(15) {
            for blue in (0..=255).step_by(15) {
                let color = Color32::from_rgb(red, green, blue);
                let picked = WheelPoint::of_oklch(Oklch::from_color(color)).to_oklch(1.0);
                assert_eq!(picked.to_color(), color, "{color:?} is not on the triangle");
            }
        }
    }
}
