use super::*;

#[test]
fn the_touch_cursor_is_one_circle_at_every_scale_and_in_css() {
    let pixel = |image: &CursorImage, x: u32, y: u32| {
        let at = ((y * image.width + x) * 4) as usize;
        [
            image.rgba[at],
            image.rgba[at + 1],
            image.rgba[at + 2],
            image.rgba[at + 3],
        ]
    };
    for (scale, size) in [(1.0, 20), (2.0, 40)] {
        let image = image(scale);
        assert_eq!((image.width, image.height), (size, size));
        assert_eq!(
            image.hotspot,
            (size / 2, size / 2),
            "the finger is its middle"
        );
        assert_eq!(image.rgba.len(), (size * size * 4) as usize);
        assert_eq!(pixel(&image, size / 2, size / 2), FILL.to_array());
        assert_eq!(pixel(&image, 0, 0)[3], 0, "outside the circle is clear");
        assert_eq!(pixel(&image, 0, size / 2)[..3], RING.to_array()[..3]);
    }
    let css = css("crosshair");
    assert!(css.starts_with("url(\"data:image/svg+xml,%3Csvg "));
    assert!(css.ends_with("\") 10 10, crosshair"), "{css}");
    assert!(!css.contains('<') && !css.contains('#'));
}
