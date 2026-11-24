use super::*;
use beui_core::font::Fonts;
use beui_types::{FontId, TextLayout};

const BELOW: &str = "every line below it wraps from the same paragraph, and none \
of their words move when the first line grows by a letter or two.";

/// Each line after the first, as its text and its glyphs' ids and places.
fn below(galley: &Galley) -> Vec<(String, Vec<(GlyphId, u32, u32)>)> {
    let lines = galley.lines();
    lines[1..]
        .iter()
        .map(|line| {
            let top = line.top * galley.pixels_per_point();
            let bottom = top + galley.line_height() * galley.pixels_per_point();
            let glyphs = galley
                .glyphs()
                .iter()
                .filter(|glyph| (top..bottom).contains(&glyph.origin.y))
                .map(|glyph| (glyph.id, glyph.origin.x.to_bits(), glyph.origin.y.to_bits()))
                .collect();
            (galley.text()[line.range.clone()].to_owned(), glyphs)
        })
        .collect()
}

#[test]
fn an_edit_on_the_first_line_leaves_the_lines_below_drawn_as_they_were() {
    let font = FontId::proportional(14.0);
    let layout = TextLayout::wrapped(220.0);
    // A fractional scale, as a Windows display at 125% has, and narrow
    // letters whose advances are fractions of a pixel.
    let scale = 1.25;
    let mut fonts = Fonts::new(SkrifaFonts::default());
    let before = fonts.layout(&format!("First line: {BELOW}"), font, layout, scale);
    assert!(
        before.lines().len() > 3,
        "the paragraph wraps onto many lines"
    );
    for typed in ["i", "il", "ilj", "W"] {
        let after = fonts.layout(&format!("First{typed} line: {BELOW}"), font, layout, scale);
        let (was, now) = (below(&before), below(&after));
        assert_eq!(
            was.iter().map(|(text, _)| text).collect::<Vec<_>>(),
            now.iter().map(|(text, _)| text).collect::<Vec<_>>(),
            "typing {typed:?} moves no word between the lines below"
        );
        assert!(
            was == now,
            "typing {typed:?} on the first line moves a glyph on a line below it"
        );
    }
}
