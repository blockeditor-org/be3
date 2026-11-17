use std::ops::Range;

use beui_text_editor_core::LineLook;
use beui_view::types::text::BLANK;
use beui_view::types::{Color, FontId, GapWidth, SpanStyle};

use super::table::unseen_width;
use super::{Builder, SpanKind, UNSEEN_SIZE};
use crate::text_area::hooks::QUOTE_INDENT_EM;

/// How many of a line's quote levels its row lays out in the quote's gutter, with their `>`
/// and the space after it, rather than indenting its text past the gutter: those of a plain
/// quote line holding its `>`s. A list, table or code block in a quote keeps its prefix in its
/// marker, its text indented past the bars.
pub fn gutter_quotes(look: &LineLook) -> usize {
    match look.list.is_none() && !look.continues && look.table.is_none() && !look.code_block {
        true => look.quote_marks,
        false => 0,
    }
}

impl Builder<'_> {
    /// Lays out a quote line's prefix over its gutter, a column per level: the `>` (with any
    /// spaces before it) unseen in the bar's span, and the space after it as a blank filling
    /// the rest of the column, so the caret stands left of a bar before its `>`, just right of
    /// it after, and at the next column or the text after the space; a level whose `>` has no
    /// space after it leaves that room as a gap. A level with no `>` on the line (a lazy
    /// continuation) is a gap of the column's width. Gives where the text after it starts.
    pub(super) fn quote_prefix(
        &mut self,
        look: &LineLook,
        line: Range<usize>,
        body: SpanStyle,
    ) -> usize {
        let bytes = &self.inputs.snapshot.bytes;
        let options = &self.inputs.options;
        let column = options.body_size * QUOTE_INDENT_EM;
        let bar = options.geometry.blocks.quote_mark.min(column);
        let unseen = SpanStyle {
            color: Color::TRANSPARENT,
            underline: false,
            strikethrough: false,
            ..body
        };
        let mark = SpanStyle {
            font: FontId::proportional(UNSEEN_SIZE),
            ..unseen
        };
        let mut at = line.start;
        for level in 0..look.quote {
            let next = at
                + bytes[at..line.end]
                    .iter()
                    .take_while(|byte| matches!(byte, b' ' | b'\t'))
                    .count();
            if level >= gutter_quotes(look) || next >= line.end || bytes[next] != b'>' {
                self.gap(GapWidth::px(column), body);
                continue;
            }
            let text = String::from_utf8_lossy(&bytes[at..next + 1]).into_owned();
            let width = unseen_width(&text, mark.font);
            self.push(&text, at..next + 1, true, mark, SpanKind::Text);
            self.gap(GapWidth::px((bar - width).max(0.0)), body);
            at = next + 1;
            match bytes.get(at).filter(|_| at < line.end) {
                Some(b' ') => {
                    self.push(BLANK, at..at + 1, true, unseen, SpanKind::Blank(column - bar));
                    at += 1;
                }
                _ => self.gap(GapWidth::px(column - bar), body),
            }
        }
        self.row.hang = self.row.display.len();
        at
    }
}
