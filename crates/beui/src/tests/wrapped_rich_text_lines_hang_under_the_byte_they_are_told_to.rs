use super::*;
use crate::reactive::*;
use beui_core::font::{FontId, Galley, TextLayout};
use beui_core::rich::RichOptions;

#[test]
fn wrapped_rich_text_lines_hang_under_the_byte_they_are_told_to() {
    let context = Context::new(crate::FreetypeFonts::default());
    let mut shaper =
        |text: &str, font: FontId| -> Galley { context.layout(text, font, TextLayout::DEFAULT) };
    let style = SpanStyle::new(FontId::proportional(14.0), Color32::WHITE);
    let text = "- one two three four five six seven eight nine ten";
    let options = RichOptions {
        wrap_width: 120.0,
        padding: (0.0, 0.0),
        style,
        hang: 2,
        anywhere: false,
        least: false,
    };
    let layout = RichLayout::new(&[Piece::text(text, style)], options, &mut shaper);

    assert!(layout.lines.len() > 1, "the text wraps");
    let content = layout.caret_rect(2, 1.0).min.x;
    assert!(content > 0.0);
    for line in &layout.lines[1..] {
        assert_eq!(
            layout.caret_rect(line.range.start, 1.0).min.x,
            content,
            "a wrapped line starts under the byte it hangs from"
        );
        assert!(
            line.width <= options.wrap_width,
            "the indent counts toward the wrap"
        );
    }
    let unhung = RichLayout::new(
        &[Piece::text(text, style)],
        RichOptions { hang: 0, ..options },
        &mut shaper,
    );
    assert_eq!(
        unhung.caret_rect(unhung.lines[1].range.start, 1.0).min.x,
        0.0
    );
}
