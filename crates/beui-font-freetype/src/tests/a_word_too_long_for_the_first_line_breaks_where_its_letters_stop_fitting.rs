use super::*;
use beui_core::font::Fonts;
use beui_types::{FontId, TextLayout};

#[test]
fn a_word_too_long_for_the_first_line_breaks_where_its_letters_stop_fitting() {
    let font = FontId::proportional(14.0);
    let mut fonts = Fonts::new(FreetypeFonts::default());
    let word = fonts
        .layout("Resizing", font, TextLayout::wrapped(f32::INFINITY), 1.0)
        .size()
        .x;
    for width in 20..word.floor() as u32 {
        let galley = fonts.layout(
            "Resizing a window reflows every paragraph in it",
            font,
            TextLayout::wrapped(width as f32),
            1.0,
        );
        let first = &galley.lines()[0];
        assert!(
            first.range.end < "Resizing".len(),
            "at {width} the first line runs on past the word that does not fit: {:?}",
            &galley.text()[first.range.clone()]
        );
        assert!(
            galley.lines().len() > 3,
            "at {width} the rest of the paragraph is left on one line"
        );
    }
}
