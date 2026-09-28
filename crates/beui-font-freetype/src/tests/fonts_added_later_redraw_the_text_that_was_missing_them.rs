use super::*;

use beui_core::font::{FontId, Fonts, TextLayout};

#[test]
fn fonts_added_later_redraw_the_text_that_was_missing_them() {
    let library = FontLibrary::new(FontSources::default());
    let mut fonts = Fonts::new(FreetypeFonts::new(library.clone()));
    let before = fonts.layout("Aa", FontId::proportional(14.0), TextLayout::DEFAULT, 1.0);
    assert!(before.glyphs().is_empty(), "with no fonts nothing is drawn");

    let generation = library.generation();
    library.replace(FontSources::bundled());
    assert_ne!(library.generation(), generation, "new fonts start a new generation");
    let after = fonts.layout("Aa", FontId::proportional(14.0), TextLayout::DEFAULT, 1.0);
    assert_eq!(after.glyphs().len(), 2, "the same text is laid out again with the new fonts");
}
