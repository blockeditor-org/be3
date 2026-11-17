use super::*;
use beui::text::{FontId, TextLayout};
use beui::text::{Piece, RichOptions};
use beui_core::font::Galley;

const LINE: &str = "afdjnafndjsk fadsnjknjafkdsl nfjakdjnklafds njafdnjlkafds njkafdsnjlkafds njkafdnjlkfads jnkafldsnlj a";

fn ranges(layout: &RichLayout) -> Vec<std::ops::Range<usize>> {
    layout.lines.iter().map(|line| line.range.clone()).collect()
}

#[test]
fn a_line_just_wide_enough_for_its_words_hangs_its_trailing_space_and_newline() {
    let context = Context::new(crate::native::FreetypeFonts::default());
    let mut shaper =
        |text: &str, font: FontId| -> Galley { context.layout(text, font, TextLayout::DEFAULT) };
    let style = SpanStyle::new(FontId::proportional(14.0), Color32::WHITE);
    let options = |wrap_width: f32| RichOptions {
        wrap_width,
        padding: (0.0, 0.0),
        style,
        hang: 0,
        anywhere: false,
    };
    let words = shaper(LINE, style.font)
        .cursor_pos(pos2(0.0, 0.0), LINE.len())
        .x;
    let end = LINE.len();

    // Whatever follows the words, at exactly their width they all stay on one line.
    for (after, lines) in [
        ("", vec![0..end]),
        ("\n", vec![0..end, end + 1..end + 1]),
        ("\nnext", vec![0..end, end + 1..end + 5]),
        (" ", vec![0..end + 1]),
        ("   ", vec![0..end + 3]),
        ("\u{2003}\u{2003}", vec![0..end + 6]),
    ] {
        let text = format!("{LINE}{after}");
        let layout = RichLayout::new(&[Piece::text(&text, style)], options(words), &mut shaper);
        assert_eq!(
            ranges(&layout),
            lines,
            "at the words' own width, {after:?} after them takes no room"
        );
        assert!(
            layout.lines[0].width <= words,
            "the first line is no wider than its words with {after:?}: {} of {words}",
            layout.lines[0].width
        );
        let caret = layout.caret_rect(end, 1.0);
        assert_eq!(
            caret.min.y, layout.lines[0].top,
            "the caret after the last word stays on its line with {after:?}"
        );
        assert!(
            caret.right() <= words + 1.0,
            "the caret after the last word stays inside the width with {after:?}: {} of {words}",
            caret.right()
        );
        let last = text.trim_end_matches(char::is_whitespace).len();
        if last == end && text.len() > end && !after.starts_with('\n') {
            let caret = layout.caret_rect(text.len(), 1.0);
            assert_eq!(
                caret.min.y, layout.lines[0].top,
                "the caret after hung space {after:?} stays on the line"
            );
            assert!(
                caret.right() <= words + 1.0,
                "the caret after hung space {after:?} stops at the edge: {} of {words}",
                caret.right()
            );
        }
    }

    // A little narrower, the last word does wrap.
    let narrower = RichLayout::new(
        &[Piece::text(&format!("{LINE}\n"), style)],
        options(words - 4.0),
        &mut shaper,
    );
    assert_eq!(
        ranges(&narrower),
        vec![0..end - 1, end - 1..end, end + 1..end + 1],
        "narrower than its words, the line wraps before the last one"
    );
}
