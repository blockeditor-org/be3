use super::*;

#[test]
fn an_oklch_color_outside_srgb_is_clamped_to_its_most_colorful_neighbour() {
    let vivid = Oklch::new(0.7, 0.4, 200.0, 1.0);
    assert!(!vivid.in_gamut());
    let clamped = vivid.clamped();
    assert!(clamped.in_gamut());
    assert_eq!(
        (clamped.lightness, clamped.hue),
        (vivid.lightness, vivid.hue)
    );
    assert!(clamped.chroma > 0.1 && clamped.chroma < vivid.chroma);
    assert_eq!(vivid.to_color(), clamped.to_color());
    let back = Oklch::from_color(vivid.to_color());
    assert!((back.lightness - 0.7).abs() < 0.01);
    assert!((back.hue - 200.0).abs() < 1.0);
    assert_eq!(
        Oklch::new(0.0, Oklch::max_chroma(0.0, 120.0), 120.0, 1.0).to_color(),
        Color32::BLACK
    );
    assert!(Oklch::max_chroma(1.0, 120.0) < 1e-3);
}
