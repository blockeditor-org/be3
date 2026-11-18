use std::ops::Range;

use beui_text_editor_core::LineLook;
use beui_view::types::text::BLANK;
use beui_view::types::{Color, FontId, GapWidth, SpanStyle};

use super::table::unseen_width;
use super::{Builder, SpanKind, UNSEEN_SIZE};
use crate::text_area::hooks::{LIST_INDENT_EM, QUOTE_INDENT_EM};

/// Whether a line's row lays out its quote's gutter itself, every level's column with the `>`
/// and space it holds, then any list nesting or a code block's padding, rather than its text
/// being indented past them: a quote line holding a `>`. The one answer the row and its line's
/// indent both follow (`TextLine::indent`, `TextLine::code_start`).
pub fn quote_in_text(look: &LineLook) -> bool {
    look.quote_marks > 0
}

/// The ems of list nesting a row whose quote is in its text lays out after the quote's
/// columns, as its line would otherwise indent it; a code line's, as a list's continuation
/// lines would be, so its block's background starts where they do (`TextLine::code_start`).
pub fn quoted_nesting_em(look: &LineLook) -> f32 {
    match look.list.is_some() || look.continues || look.code_block {
        true => (look.depth + usize::from(look.continues)) as f32 * LIST_INDENT_EM,
        false => 0.0,
    }
}

impl Builder<'_> {
    /// Lays out a quote line's prefix over its gutter, a column per level: the `>` (with any
    /// spaces before it) unseen in the bar's span, and the space after it as a blank filling
    /// the rest of the column, so the caret stands left of a bar before its `>`, just right of
    /// it after, and at the next column or the text after the space; a level whose `>` has no
    /// space after it leaves that room as a gap. A level with no `>` on the line (a lazy
    /// continuation) is a gap of the column's width. Then any list nesting, and a code line's
    /// padding inside its block's background, as gaps. `body` is the style of the line's text,
    /// a code line's monospace, so the prefix's spans make the line no taller. Gives where the
    /// text after it starts, and how wide it is.
    pub(super) fn quote_prefix(
        &mut self,
        look: &LineLook,
        line: Range<usize>,
        body: SpanStyle,
    ) -> (usize, f32) {
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
            if level >= look.quote_marks || next >= line.end || bytes[next] != b'>' {
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
                    self.push(
                        BLANK,
                        at..at + 1,
                        true,
                        unseen,
                        SpanKind::Blank(column - bar),
                    );
                    at += 1;
                }
                _ => self.gap(GapWidth::px(column - bar), body),
            }
        }
        let nesting = quoted_nesting_em(look) * options.body_size;
        let padding = match look.code_block {
            true => options.code_padding,
            false => 0.0,
        };
        for width in [nesting, padding] {
            if width > 0.0 {
                self.gap(GapWidth::px(width), body);
            }
        }
        self.row.hang = self.row.display.len();
        (at, look.quote as f32 * column + nesting + padding)
    }
}
