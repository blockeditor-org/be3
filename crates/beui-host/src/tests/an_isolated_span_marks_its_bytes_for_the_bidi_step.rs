use super::*;
use beui::text::{FontId, TextLayout};
use beui::text::{Piece, RichOptions};
use beui_core::font::Galley;

#[test]
fn an_isolated_span_marks_its_bytes_for_the_bidi_step() {
    let context = Context::new(crate::native::FreetypeFonts::default());
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
    let isolated = |text| Piece::Text {
        text,
        style,
        break_after: false,
        isolate: true,
    };
    let layout = RichLayout::new(
        &[
            Piece::text("Leave \u{201c}", style),
            isolated("Team\tA"),
            isolated(""),
            isolated("B"),
            Piece::text("\u{201d}?", style),
        ],
        options,
        &mut shaper,
    );
    assert_eq!(
        layout.isolates,
        vec![9..15, 15..16],
        "each isolated piece is one run, tabs and all, and an empty one is none"
    );
    let plain = RichLayout::new(&[Piece::text("Leave", style)], options, &mut shaper);
    assert!(plain.isolates.is_empty());
}
