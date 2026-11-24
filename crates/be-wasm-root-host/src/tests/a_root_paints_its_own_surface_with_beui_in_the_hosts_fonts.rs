use be_sdk_root_testing::{ROOT_COLOR, ROOT_SURFACE, TEXT_STRIP, TEXT_SURFACE};

use super::*;

#[test]
fn a_root_paints_its_own_surface_with_beui_in_the_hosts_fonts() {
    let mut root = embedding();
    root.frame(
        FrameRequest {
            time: 1,
            wall_clock: 0,
            utc_offset: 0,
            window: window(),
        },
        Instant::now() + PATIENT,
    )
    .expect("the root draws");
    let paintings = own_paintings(&mut root, 1);
    let surfaces: Vec<u64> = paintings.iter().map(|(surface, _)| *surface).collect();
    assert_eq!(
        surfaces,
        vec![u64::from(ROOT_SURFACE), u64::from(TEXT_SURFACE)],
        "the root paints each surface of its own"
    );
    let own = ROOT_COLOR.map(|channel| (channel * 255.0).round() as u8);
    assert_eq!(
        paintings[0].1.frame.background, own,
        "its own surface is the colour it fills it with"
    );
    let text = &paintings[1].1;
    assert_eq!(
        text.frame.background[3], 0,
        "beui leaves its strip clear but for the text, so the root's colour shows through"
    );
    let glyphs = glyphs(&text.frame.primitives);
    assert!(
        !glyphs.is_empty(),
        "its line of text is drawn in the fonts the host gave, so it paints glyphs"
    );
    let (width, height) = (TEXT_STRIP.0 as f32, TEXT_STRIP.1 as f32);
    for [left, top, right, bottom] in glyphs {
        assert!(
            left >= 0.0 && top >= 0.0 && right <= width && bottom <= height,
            "the line lies within its strip: a glyph at {left}..{right}, {top}..{bottom}"
        );
    }
}
