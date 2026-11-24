//! Evidence at the layout level for the text area's regression test,
//! be-app-text's `a_line_just_wide_enough_keeps_its_last_word_and_its_newline`.
use super::*;
use beui::text::{FontId, TextLayout};
use beui::text::{Piece, RichOptions};
use beui_core::font::Galley;

const LINE: &str = "afdjnafndjsk fadsnjknjafkdsl nfjakdjnklafds njafdnjlkafds njkafdsnjlkafds njkafdnjlkfads jnkafldsnlj a";
const BLANK: f32 = 14.0;

fn ranges(layout: &RichLayout) -> Vec<std::ops::Range<usize>> {
    layout.lines.iter().map(|line| line.range.clone()).collect()
}

#[test]
fn a_line_just_wide_enough_for_its_words_hangs_its_trailing_space_and_newline() {
    let context = Context::new(crate::native::SkrifaFonts::default());
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
    for (after, blank, lines) in [
        ("", false, vec![0..end]),
        ("", true, vec![0..end + 1]),
        ("\n", false, vec![0..end, end + 1..end + 1]),
        ("\nnext", false, vec![0..end, end + 1..end + 5]),
        (" ", false, vec![0..end + 1]),
        ("   ", false, vec![0..end + 3]),
        ("  ", true, vec![0..end + 3]),
    ] {
        let text = format!("{LINE}{after}");
        let mut pieces = vec![Piece::text(&text, style)];
        if blank {
            pieces.push(Piece::Blank {
                width: BLANK,
                style,
            });
        }
        let layout = RichLayout::new(&pieces, options(words), &mut shaper);
        let case = format!("{after:?}{}", if blank { " and a blank" } else { "" });
        assert_eq!(
            ranges(&layout),
            lines,
            "at the words' own width, {case} after them takes no room"
        );
        assert!(
            layout.lines[0].width <= words,
            "the first line is no wider than its words with {case}: {} of {words}",
            layout.lines[0].width
        );
        let caret = layout.caret_rect(end, 1.0);
        assert_eq!(
            caret.min.y, layout.lines[0].top,
            "the caret after the last word stays on its line with {case}"
        );
        assert!(
            caret.right() <= words + 1.0,
            "the caret after the last word stays inside the width with {case}: {} of {words}",
            caret.right()
        );
        if !after.starts_with('\n') {
            let last = layout.lines[0].range.end;
            let caret = layout.caret_rect(last, 1.0);
            assert_eq!(
                caret.min.y, layout.lines[0].top,
                "the caret after hung {case} stays on the line"
            );
            assert!(
                caret.right() <= words + 1.0,
                "the caret after hung {case} stops at the edge: {} of {words}",
                caret.right()
            );
        }
        if blank {
            // A blank keeps its width past the edge, after any spaces hung at the edge, so a
            // mark over it has room beside the last word.
            let at = layout.lines[0].range.end - 1;
            let boxes = layout.selection_rects(at..at + 1);
            assert_eq!(boxes.len(), 1, "the blank is one box with {case}");
            assert!(
                (boxes[0].min.x - words).abs() < 1.0 && (boxes[0].width() - BLANK).abs() < 0.01,
                "the blank's box starts at the edge and keeps its width with {case}: {:?}",
                boxes[0]
            );
        }
    }

    // A little narrower, the last word does wrap, and the blank with it.
    let text = format!("{LINE}");
    let narrower = RichLayout::new(
        &[
            Piece::text(&text, style),
            Piece::Blank {
                width: BLANK,
                style,
            },
        ],
        options(words - 4.0),
        &mut shaper,
    );
    assert_eq!(
        ranges(&narrower),
        vec![0..end - 1, end - 1..end + 1],
        "narrower than its words, the line wraps before the last one"
    );
    let caret = narrower.caret_rect(end, 1.0);
    assert_eq!(
        caret.min.y, narrower.lines[1].top,
        "the caret before the blank is on the wrapped row"
    );
}
