use super::*;
use beui::text::{FontId, TextLayout};
use beui::text::{Piece, RichOptions};
use beui_core::font::Galley;

fn ranges(layout: &RichLayout) -> Vec<std::ops::Range<usize>> {
    layout.lines.iter().map(|line| line.range.clone()).collect()
}

#[test]
fn a_break_in_rich_text_ends_the_line_but_holds_no_bytes() {
    let context = Context::new(crate::native::FreetypeFonts::default());
    let mut shaper =
        |text: &str, font: FontId| -> Galley { context.layout(text, font, TextLayout::DEFAULT) };
    let style = SpanStyle::new(FontId::monospace(14.0), Color32::WHITE);
    let options = |wrap_width: f32| RichOptions {
        wrap_width,
        padding: (0.0, 0.0),
        style,
        hang: 0,
        anywhere: false,
        least: false,
    };
    let pieces = [
        Piece::text("abc", style),
        Piece::Gap(7.0),
        Piece::Break,
        Piece::Gap(11.0),
        Piece::text("def", style),
    ];
    let abc = shaper("abc", style.font).cursor_pos(pos2(0.0, 0.0), 3).x;
    let def = shaper("def", style.font).cursor_pos(pos2(0.0, 0.0), 3).x;

    for wrap in [f32::INFINITY, 400.0] {
        let layout = RichLayout::new(&pieces, options(wrap), &mut shaper);
        assert_eq!(
            ranges(&layout),
            vec![0..3, 3..6],
            "a break ends the line at {wrap} and holds no bytes"
        );
        assert_eq!(
            layout.lines[0].width,
            abc + 7.0,
            "the gap before the break stays on the first line"
        );
        assert_eq!(
            layout.lines[1].width,
            11.0 + def,
            "the gap after the break starts the second line"
        );
        assert_eq!(
            layout.caret_rect(3, 1.0).min.y,
            layout.lines[1].top,
            "the caret at the break stands at the start of the next line"
        );
        assert_eq!(layout.selection_rects(2..4).len(), 2);
    }

    let doubled = RichLayout::new(
        &[
            Piece::text("ab", style),
            Piece::Break,
            Piece::Break,
            Piece::text("cd\nef", style),
            Piece::Break,
        ],
        options(f32::INFINITY),
        &mut shaper,
    );
    assert_eq!(
        ranges(&doubled),
        vec![0..2, 2..2, 2..4, 5..7, 7..7],
        "two breaks leave an empty line between them, and a last one an empty line after it"
    );
}
