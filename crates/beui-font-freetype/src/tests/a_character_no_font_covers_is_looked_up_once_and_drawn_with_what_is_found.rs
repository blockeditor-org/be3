use super::*;
use std::cell::Cell;
use std::rc::Rc;

use beui_core::font::{FontId, Fonts, TextLayout};

#[test]
fn a_character_no_font_covers_is_looked_up_once_and_drawn_with_what_is_found() {
    let emoji = FontSources::bundled().fallback[0].clone();
    let sources = FontSources {
        fallback: Vec::new(),
        ..FontSources::bundled()
    };
    let asked = Rc::new(Cell::new(0));
    let counter = asked.clone();
    let library = FontLibrary::new(sources).with_fallback(move |character| {
        counter.set(counter.get() + 1);
        (character == '\u{1f600}').then(|| emoji.clone())
    });
    let mut fonts = Fonts::new(FreetypeFonts::new(library));

    let galley = fonts.layout(
        "a \u{1f600}",
        FontId::proportional(14.0),
        TextLayout::DEFAULT,
        1.0,
    );
    assert_eq!(asked.get(), 1, "only the character no font covered was looked up");
    assert_eq!(
        galley.glyphs().len(),
        2,
        "the letter and the emoji are both drawn, the emoji from the font the lookup found"
    );

    fonts.layout("\u{1f600}\u{1f600}", FontId::proportional(14.0), TextLayout::DEFAULT, 1.0);
    assert_eq!(asked.get(), 1, "a font once found is kept for every later galley");
}
