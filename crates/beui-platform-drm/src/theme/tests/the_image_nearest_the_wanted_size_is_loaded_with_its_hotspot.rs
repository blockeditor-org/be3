use super::*;

#[test]
fn the_image_nearest_the_wanted_size_is_loaded_with_its_hotspot() {
    let icons = Icons::new("sizes");
    icons.cursor(
        "default",
        "xterm",
        &[
            drawn(24, (11, 12), 0xff11_2233),
            drawn(48, (22, 24), 0x8000_0080),
            drawn(64, (30, 31), 0xffff_ffff),
        ],
    );
    let theme = icons.theme("default", 24);

    let small = theme
        .load(&["text", "xterm"], 24)
        .expect("a later name is tried when the first is missing");
    assert_eq!((small.nominal, small.width, small.height), (24, 24, 24));
    assert_eq!(small.hotspot, (11, 12));
    assert_eq!(&small.rgba[..4], &[0x11, 0x22, 0x33, 0xff]);

    let doubled = theme.load(&["xterm"], 50).expect("the cursor loads");
    assert_eq!(doubled.nominal, 48, "the nearest size is chosen");
    assert_eq!(doubled.hotspot, (22, 24));
    assert_eq!(
        &doubled.rgba[..4],
        &[0, 0, 0x80, 0x80],
        "the pixels stay premultiplied"
    );

    assert_eq!(theme.load(&["text"], 24), None);
}
