use super::*;

#[test]
fn a_color_round_trips_through_hsv() {
    for red in (0..=255).step_by(17) {
        for green in (0..=255).step_by(51) {
            for blue in (0..=255).step_by(85) {
                let color = Color32::from_rgba_unmultiplied(red, green, blue, 128);
                assert_eq!(Hsva::from_color(color).to_color(), color);
            }
        }
    }
    let red = Hsva::from_color(Color32::from_rgb(255, 0, 0));
    assert_eq!((red.hue, red.saturation, red.value), (0.0, 1.0, 1.0));
    let blue = Hsva::from_color(Color32::from_rgb(0, 0, 255));
    assert_eq!(blue.hue, 240.0);
}
