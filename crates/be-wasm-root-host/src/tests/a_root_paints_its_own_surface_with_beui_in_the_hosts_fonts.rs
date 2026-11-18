use be_sdk_root_testing::{ORIGIN, ROOT_COLOR, ROOT_SURFACE, TEXT_ORIGIN};
use beui_paint_format::{Content, Primitive, ScreenPaintings};

use super::*;

/// Every glyph `primitives` paint, in groups too.
fn glyphs(primitives: &[Primitive]) -> Vec<[f32; 4]> {
    primitives
        .iter()
        .flat_map(|primitive| match &primitive.content {
            Content::Glyph(glyph) => vec![glyph.rect],
            Content::Group(group) => glyphs(&group.primitives),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn a_root_paints_its_own_surface_with_beui_in_the_hosts_fonts() {
    let mut root = embedding();
    root.frame(FrameRequest { time: 1 }, Instant::now() + PATIENT)
        .expect("the root draws");
    root.ask_paintings().expect("the root runs");
    root.frame(FrameRequest { time: 2 }, Instant::now() + PATIENT)
        .expect("the root draws");
    let painted = root
        .take_paintings()
        .expect("the root answered with the frame it stepped through");
    let painted = ScreenPaintings::decode(&painted).expect("the root's paintings decode");
    let [(surface, painting)] = &painted.screens[..] else {
        panic!(
            "the root paints its one surface of its own, not {:?}",
            painted.screens.len()
        );
    };
    assert_eq!(*surface, u64::from(ROOT_SURFACE));
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
