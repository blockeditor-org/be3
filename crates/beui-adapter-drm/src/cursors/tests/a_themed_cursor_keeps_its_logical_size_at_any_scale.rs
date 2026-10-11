use super::*;

#[test]
fn a_themed_cursor_keeps_its_logical_size_at_any_scale() {
    let icons = Icons::new("scaled");
    icons.cursor(
        "default",
        "grab",
        &[
            drawn(24, (8, 4), 0xffff_ffff),
            drawn(48, (16, 8), 0xffff_ffff),
        ],
    );
    let theme = icons.theme("default", 24);

    for (scale, nominal) in [(1.0, 24), (2.0, 48), (1.5, 48), (3.0, 48)] {
        let Choice::Themed {
            image,
            size,
            hotspot,
        } = choose(&theme, CursorIcon::Grab, scale)
        else {
            panic!("the grab cursor is themed");
        };
        assert_eq!(
            image.nominal, nominal,
            "at {scale}x the sharpest image is used"
        );
        assert_eq!(
            size,
            vec2(24.0, 24.0),
            "at {scale}x the cursor is 24 points"
        );
        assert_eq!(hotspot, vec2(8.0, 4.0), "at {scale}x the hotspot stays put");
    }
}
