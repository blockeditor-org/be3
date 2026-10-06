use super::*;
use beui_core::font::{FontId, Fonts, TextLayout};

#[test]
fn a_colour_bitmap_glyph_is_scaled_to_the_text_in_its_own_colours() {
    let sources = FontSources {
        fallback: vec![FontData::from_static(include_bytes!(
            "../../assets/test/ColourBitmap.ttf"
        ))],
        ..FontSources::bundled()
    };
    let mut fonts = Fonts::new(FreetypeFonts::new(FontLibrary::new(sources)));

    let galley = fonts.layout(
        "\u{1f600}",
        FontId::proportional(16.0),
        TextLayout::DEFAULT,
        1.0,
    );
    let [glyph] = galley.glyphs() else {
        panic!("the emoji is drawn as one glyph");
    };
    let image = &glyph.image;
    assert!(
        image.color,
        "a glyph from a colour bitmap is drawn in colour"
    );
    assert_eq!(
        (image.width, image.height),
        (16, 16),
        "the 32 pixel strike is scaled to the 16 pixel text"
    );
    assert_eq!(
        (image.left, image.top),
        (0, 13),
        "the strike's bearings are scaled with it"
    );
    let pixel = |x: u32, y: u32| {
        let at = ((y * image.width + x) * 4) as usize;
        [
            image.pixels[at],
            image.pixels[at + 1],
            image.pixels[at + 2],
            image.pixels[at + 3],
        ]
    };
    assert_eq!(pixel(8, 2), [255, 0, 0, 255], "the top half stays red");
    assert_eq!(pixel(8, 13), [0, 0, 255, 255], "the bottom half stays blue");
}
