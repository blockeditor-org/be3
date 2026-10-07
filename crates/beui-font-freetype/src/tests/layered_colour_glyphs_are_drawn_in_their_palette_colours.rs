use super::*;
use beui_core::font::{FontId, Fonts, TextLayout};

#[test]
fn layered_colour_glyphs_are_drawn_in_their_palette_colours() {
    let sources = FontSources {
        fallback: vec![FontData::from_static(include_bytes!(
            "../../assets/test/ColourLayers.ttf"
        ))],
        ..FontSources::bundled()
    };
    let mut fonts = Fonts::new(FreetypeFonts::new(FontLibrary::new(sources)));

    let galley = fonts.layout(
        "\u{1f600}",
        FontId::proportional(32.0),
        TextLayout::DEFAULT,
        1.0,
    );
    let [glyph] = galley.glyphs() else {
        panic!("the emoji is drawn as one glyph");
    };
    let image = &glyph.image;
    assert!(
        image.color,
        "a glyph made of coloured layers is drawn in colour"
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
    let middle = image.height / 2;
    assert_eq!(
        pixel(image.width / 4, middle),
        [255, 0, 0, 255],
        "the left layer is painted in the palette's first colour"
    );
    assert_eq!(
        pixel(image.width * 3 / 4, middle),
        [0, 0, 255, 255],
        "the right layer is painted in the palette's second colour"
    );
}
