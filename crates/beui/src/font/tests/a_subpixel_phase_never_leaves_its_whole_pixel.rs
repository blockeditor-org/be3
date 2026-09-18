use super::*;

#[test]
fn a_subpixel_phase_never_leaves_its_whole_pixel() {
    let positions = SUBPIXEL_POSITIONS as f32;
    let mut x = -4.0;
    while x < 4.0 {
        let (whole, subpixel) = split_subpixel(x);

        assert_eq!(whole, whole.floor(), "the whole part of {x} is fractional");
        assert!(subpixel < SUBPIXEL_POSITIONS, "{x} chose phase {subpixel}");
        assert!(
            (whole + subpixel as f32 / positions - x).abs() <= 0.5 / positions,
            "{x} landed more than half a phase away"
        );

        x += 1.0 / 32.0;
    }
}
