use super::*;
use beui_core::font::Fonts;
use beui_types::{FontId, TextLayout};

const TEXT: &str = "A paragraph wraps onto several lines, and laid out again at the \
width it measured, which a column that fits its content gives it, it breaks \
exactly where it did: each line's width, shaped on its own, never exceeds what \
the breaks were chosen against.";

#[test]
fn a_wrapped_text_laid_out_at_its_own_width_keeps_its_lines() {
    let font = FontId::proportional(14.0);
    for scale in [1.0, 1.25, 1.5, 2.0] {
        for wrap in (120..420).step_by(7) {
            let mut fonts = Fonts::new(SkrifaFonts::default());
            let wrapped = fonts.layout(TEXT, font, TextLayout::wrapped(wrap as f32), scale);
            let lines = |galley: &Galley| {
                galley
                    .lines()
                    .iter()
                    .map(|line| line.range.clone())
                    .collect::<Vec<_>>()
            };
            let width = wrapped.size().x;
            let again = Fonts::new(SkrifaFonts::default()).layout(
                TEXT,
                font,
                TextLayout::wrapped(width),
                scale,
            );
            assert_eq!(
                lines(&wrapped),
                lines(&again),
                "at {wrap} and scale {scale}, laid out again at its own width {width}"
            );
        }
    }
}
