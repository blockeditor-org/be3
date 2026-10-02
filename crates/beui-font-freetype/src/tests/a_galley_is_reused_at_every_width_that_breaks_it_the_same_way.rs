use super::*;
use beui_core::font::{FontId, Fonts, TextLayout};

const TEXT: &str = "Resizing a window reflows every paragraph in it, and most widths \
break most paragraphs exactly where the previous width did.\nA second line, with a \
supercalifragilisticexpialidocious word too long for the narrowest widths.";

type Breaks = (Vec<Range<usize>>, Vec<(GlyphId, f32, f32)>);

fn breaks(galley: &Galley) -> Breaks {
    (
        galley
            .lines()
            .iter()
            .map(|line| line.range.clone())
            .collect(),
        galley
            .glyphs()
            .iter()
            .map(|glyph| (glyph.id, glyph.offset.x, glyph.offset.y))
            .collect(),
    )
}

#[test]
fn a_galley_is_reused_at_every_width_that_breaks_it_the_same_way() {
    let font = FontId::proportional(14.0);
    let mut reused = Fonts::new(FreetypeFonts::default());
    let mut previous: Option<(Galley, Breaks)> = None;

    for width in (40..600).chain((40..600).rev()) {
        let layout = TextLayout::wrapped(width as f32);
        let galley = reused.layout(TEXT, font, layout, 1.0);
        let fresh = breaks(&Fonts::new(FreetypeFonts::default()).layout(TEXT, font, layout, 1.0));
        assert!(
            breaks(&galley) == fresh,
            "at {width} the reused galley breaks differently from a fresh one"
        );
        if let Some((galley_before, breaks_before)) = &previous {
            assert_eq!(
                *galley_before == galley,
                *breaks_before == fresh,
                "at {width} a new galley must be built exactly when the breaks change"
            );
        }
        previous = Some((galley, fresh));
    }
}
