use super::*;

use beui_core::font::{FontId, Fonts, TextLayout};

#[test]
fn replacing_the_fonts_lays_the_text_out_again() {
    let empty = FreetypeFonts::new(FontLibrary::new(FontSources::default()));
    let mut fonts = Fonts::new(empty);
    let generation = fonts.generation();
    let before = fonts.layout("Aa", FontId::proportional(14.0), TextLayout::DEFAULT, 1.0);
    assert!(before.glyphs().is_empty(), "with no fonts nothing is drawn");

    fonts.replace(Box::new(FreetypeFonts::default()));
    assert_ne!(
        fonts.generation(),
        generation,
        "other fonts start a new generation"
    );
    let after = fonts.layout("Aa", FontId::proportional(14.0), TextLayout::DEFAULT, 1.0);
    assert_eq!(
        after.glyphs().len(),
        2,
        "the same text is laid out again with the other fonts"
    );
}
