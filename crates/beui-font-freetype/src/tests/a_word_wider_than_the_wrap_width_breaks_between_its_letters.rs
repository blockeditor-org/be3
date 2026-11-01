use super::*;
use beui_core::font::Fonts;
use beui_types::{FontId, TextLayout};

#[test]
fn a_word_wider_than_the_wrap_width_breaks_between_its_letters() {
    let font = FontId::proportional(14.0);
    let mut fonts = Fonts::new(FreetypeFonts::default());
    let galley = fonts.layout("Wide", font, TextLayout::wrapped(1.0), 1.0);

    let lines: Vec<&str> = galley
        .lines()
        .iter()
        .map(|line| &galley.text()[line.range.clone()])
        .collect();
    assert_eq!(lines, ["W", "i", "d", "e"]);
}
