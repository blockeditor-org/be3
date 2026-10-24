use super::*;
use beui_core::font::{FontFamily, FontId, Fonts, TextLayout};

#[test]
fn every_glyph_draws_inside_the_box_it_is_culled_by() {
    let sources = FontSources {
        fallback: vec![
            FontData::from_static(include_bytes!("fonts/ColourBitmap.ttf")),
            FontData::from_static(include_bytes!("fonts/ColourLayers.ttf")),
        ],
        ..FontSources::bundled()
    };
    let mut fonts = Fonts::new(FreetypeFonts::new(FontLibrary::new(sources)));
    let text = "Quick fjord gyp! WAVY {[|]} 0123 \u{1f600}";
    let icons: String = [beui_core::icons::ICON_CODE, beui_core::icons::ICON_BLOCKS]
        .map(beui_core::icons::Glyph::as_str)
        .concat();
    for size in [9.0, 13.0, 16.0, 24.0, 40.0] {
        for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
            for (family, text) in [
                (FontFamily::Proportional, text),
                (FontFamily::Monospace, text),
                (FontFamily::Icons, icons.as_str()),
            ] {
                let font = FontId::new(size, family).bold(bold).italic(italic);
                let galley = fonts.layout(text, font, TextLayout::DEFAULT, 1.0);
                for glyph in galley.glyphs() {
                    let (Some(image), Some(offset)) = (glyph.image(), glyph.offset()) else {
                        continue;
                    };
                    if image.width == 0 || image.height == 0 {
                        continue;
                    }
                    let [left, top, right, bottom] = glyph.bounds();
                    let drawn = [
                        offset.x,
                        offset.y,
                        offset.x + image.width as f32,
                        offset.y + image.height as f32,
                    ];
                    assert!(
                        left <= drawn[0]
                            && top <= drawn[1]
                            && drawn[2] <= right
                            && drawn[3] <= bottom,
                        "glyph {:?} of {text:?} in {font:?} draws at {drawn:?}, outside the box {:?} it is culled by",
                        glyph.id,
                        glyph.bounds(),
                    );
                }
            }
        }
    }
}
