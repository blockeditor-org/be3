use super::*;
use beui_core::font::{FontId, Fonts, TextLayout};

#[test]
fn a_word_that_fits_stays_on_its_line() {
    let font = FontId::proportional(14.0);
    let mut fonts = Fonts::new(FreetypeFonts::default());
    let fitted = fonts
        .layout("one two ", font, TextLayout::DEFAULT, 1.0)
        .size()
        .x;
    let galley = fonts.layout("one two three", font, TextLayout::wrapped(fitted), 1.0);

    let lines: Vec<&str> = galley
        .lines()
        .iter()
        .map(|line| &galley.text()[line.range.clone()])
        .collect();
    assert_eq!(lines, ["one two ", "three"]);
}
