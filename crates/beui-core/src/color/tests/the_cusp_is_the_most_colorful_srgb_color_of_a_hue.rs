use super::*;

#[test]
fn the_cusp_is_the_most_colorful_srgb_color_of_a_hue() {
    let red = Oklch::cusp(Oklch::from_color(Color32::from_rgb(255, 0, 0)).hue);
    assert_eq!(red.to_color(), Color32::from_rgb(255, 0, 0));
    let yellow = Oklch::cusp(Oklch::from_color(Color32::from_rgb(255, 255, 0)).hue);
    assert!((yellow.lightness - 0.968).abs() < 0.005, "{yellow:?}");
    for hue in (0..360).step_by(15) {
        let cusp = Oklch::cusp(hue as f32);
        for step in 1..20 {
            let lightness = step as f32 / 20.0;
            assert!(Oklch::max_chroma(lightness, hue as f32) <= cusp.chroma + 1e-4);
        }
    }
}
