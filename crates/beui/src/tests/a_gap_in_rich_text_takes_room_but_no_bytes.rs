use super::*;
use crate::reactive::*;
use beui_core::font::{FontId, Galley, TextLayout};
use beui_core::rich::RichOptions;

#[test]
fn a_gap_in_rich_text_takes_room_but_no_bytes() {
    let context = Context::new(crate::FreetypeFonts::default());
    let mut shaper =
        |text: &str, font: FontId| -> Galley { context.layout(text, font, TextLayout::DEFAULT) };
    let style = SpanStyle::new(FontId::proportional(14.0), Color32::WHITE);
    let options = RichOptions {
        wrap_width: f32::INFINITY,
        padding: (0.0, 0.0),
        style,
        hang: 0,
        anywhere: false,
        least: false,
    };
    let plain = RichLayout::new(
        &[Piece::text("ab", style), Piece::text("cd", style)],
        options,
        &mut shaper,
    );
    let gapped = RichLayout::new(
        &[
            Piece::text("ab", style),
            Piece::Gap(30.0),
            Piece::text("cd", style),
            Piece::Gap(5.0),
        ],
        options,
        &mut shaper,
    );

    assert_eq!(gapped.lines[0].range, 0..4, "a gap holds no bytes");
    assert_eq!(
        gapped.caret_rect(2, 1.0).min.x,
        plain.caret_rect(2, 1.0).min.x + 30.0,
        "the text after a gap starts past it"
    );
    assert_eq!(
        gapped.size.x,
        plain.size.x + 35.0,
        "gaps, the last one too, widen the line"
    );
}
