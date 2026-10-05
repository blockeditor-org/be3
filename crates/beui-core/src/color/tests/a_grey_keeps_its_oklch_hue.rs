use super::*;

#[test]
fn a_grey_keeps_its_oklch_hue() {
    let held = Oklch::new(0.5, 0.1, 250.0, 1.0);
    let grey = Oklch::from_color_keeping(Color32::from_gray(90), held);
    assert_eq!(grey.hue, 250.0);
    assert!(grey.chroma < 1e-3);
    let outside = Oklch::new(0.6, 0.45, 140.0, 1.0);
    assert_eq!(
        Oklch::from_color_keeping(outside.to_color(), outside),
        outside
    );
}
