use super::*;

#[test]
fn a_color_round_trips_through_oklch() {
    for red in (0..=255).step_by(17) {
        for green in (0..=255).step_by(51) {
            for blue in (0..=255).step_by(85) {
                let color = Color32::from_rgba_unmultiplied(red, green, blue, 128);
                assert_eq!(Oklch::from_color(color).to_color(), color);
            }
        }
    }
    let white = Oklch::from_color(Color32::WHITE);
    assert!((white.lightness - 1.0).abs() < 1e-3);
    assert!(white.chroma < 1e-3);
    let red = Oklch::from_color(Color32::from_rgb(255, 0, 0));
    assert!((red.lightness - 0.628).abs() < 1e-3);
    assert!((red.chroma - 0.258).abs() < 1e-3);
    assert!((red.hue - 29.23).abs() < 0.1);
}
