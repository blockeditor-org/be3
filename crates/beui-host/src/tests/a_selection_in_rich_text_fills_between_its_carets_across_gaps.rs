use super::*;
use beui::text::{FontId, TextLayout};
use beui::text::{GapWidth, Piece, RichOptions};
use beui_core::font::Galley;

/// A selection is drawn from where a caret at its start stands to where one at its end does:
/// it covers a gap inside it and a gap its end's caret stands past, at the end of a line too,
/// so what is selected is exactly what lies between the carets.
#[test]
fn a_selection_in_rich_text_fills_between_its_carets_across_gaps() {
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
    };
    let layout = RichLayout::new(
        &[
            Piece::text("a", style),
            Piece::Gap(GapWidth::px(30.0)),
            Piece::text("b", style),
            Piece::Gap(GapWidth::px(20.0)),
        ],
        options,
        &mut shaper,
    );
    let caret = |at: usize| layout.caret_rect(at, 0.0).min.x;
    let fill = |range: std::ops::Range<usize>| {
        let rects = layout.selection_rects(range);
        assert_eq!(rects.len(), 1);
        (rects[0].min.x, rects[0].max.x)
    };

    assert_eq!(
        fill(0..1),
        (caret(0), caret(1)),
        "the gap after a is covered"
    );
    assert_eq!(caret(1), caret(0) + layout.lines[0].runs[0].width + 30.0);
    assert_eq!(fill(1..2), (caret(1), caret(2)));
    assert_eq!(
        caret(2),
        layout.size.x,
        "a caret at the line's end stands past the gap that ends it"
    );
    assert_eq!(fill(0..2), (0.0, layout.size.x));
}
