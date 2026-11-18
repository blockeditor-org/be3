use be_sdk_root_testing::{ORIGIN, ROOT_COLOR, TEXT_ORIGIN};

use super::*;

#[test]
fn a_root_paints_its_own_surface_with_beui_in_the_hosts_fonts() {
    let mut root = embedding();
    root.frame(FrameRequest { time: 1, wall_clock: 0 }, Instant::now() + PATIENT)
        .expect("the root draws");
    let painting = own_painting(&mut root, 1);
    let own = ROOT_COLOR.map(|channel| (channel * 255.0).round() as u8);
    assert_eq!(
        painting.frame.background,
        own,
        "its beui document clears the surface to the root's colour"
    );
    let glyphs = glyphs(&painting.frame.primitives);
    assert!(
        !glyphs.is_empty(),
        "its line of text is drawn in the fonts the host gave, so it paints glyphs"
    );
    for [left, top, _, bottom] in glyphs {
        assert!(
            left >= TEXT_ORIGIN.0 - 1.0 && top >= 0.0 && bottom <= ORIGIN.1,
            "the line lies in the strip above the triangle's screen: a glyph at {left}, \
             {top}..{bottom}"
        );
    }
}
