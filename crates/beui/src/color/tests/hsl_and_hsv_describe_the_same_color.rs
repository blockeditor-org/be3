use super::*;

#[test]
fn hsl_and_hsv_describe_the_same_color() {
    let red = Hsva::from_hsl(0.0, 1.0, 0.5, 1.0);
    assert_eq!(red.to_color(), Color32::from_rgb(255, 0, 0));
    let pale = Hsva::from_hsl(210.0, 0.5, 0.75, 1.0);
    let [hue, saturation, lightness] = pale.hsl();
    assert!((hue - 210.0).abs() < 1e-4);
    assert!((saturation - 0.5).abs() < 1e-4);
    assert!((lightness - 0.75).abs() < 1e-4);
    assert_eq!(
        Hsva::from_hsl(90.0, 0.3, 1.0, 1.0).to_color(),
        Color32::WHITE
    );
    assert_eq!(
        Hsva::from_hsl(90.0, 0.3, 0.0, 1.0).to_color(),
        Color32::BLACK
    );
}
