use std::ops::Range;

use beui_view::types::{Color, FontId, GapWidth, SpanStyle};

use super::table::{text_width, unseen_width};
use super::{Builder, SpanKind, UNSEEN_SIZE};

/// Room each of a rule's characters leaves, so the browser's measure of a character, which may
/// differ a little from this one, never pushes the rule onto a second line.
const RULE_SLACK: f32 = 0.5;

impl Builder<'_> {
    /// A thematic break's characters, unseen: the rule is drawn in their place. When `shared`
    /// (a rule with no marker before it) each takes an equal share of the row's width, so the
    /// caret's stops divide the rule evenly and it steps across a character at a time. Gives
    /// the spans it pushed, for `fit_rule`.
    pub(super) fn rule(
        &mut self,
        characters: Range<usize>,
        shared: bool,
        body: SpanStyle,
    ) -> Range<usize> {
        let bytes = &self.inputs.snapshot.bytes;
        let hidden = SpanStyle {
            color: Color::TRANSPARENT,
            underline: false,
            strikethrough: false,
            ..body
        };
        let hidden = match shared {
            true => SpanStyle {
                font: FontId::proportional(UNSEEN_SIZE),
                ..hidden
            },
            false => hidden,
        };
        let first = self.row.spans.len();
        let share = 1.0 / characters.len().max(1) as f32;
        for at in characters {
            let text = String::from_utf8_lossy(&bytes[at..at + 1]).into_owned();
            let width = match shared {
                true => unseen_width(&text, hidden.font),
                false => text_width(&text, hidden.font),
            };
            self.push(&text, at..at + 1, true, hidden, SpanKind::Text);
            if shared {
                let px = -width - RULE_SLACK;
                self.gap(GapWidth { px, share }, body);
            }
        }
        first..self.row.spans.len()
    }

    /// Makes room after a rule's shares for what the row lays out after its characters
    /// (trailing spaces, a carriage return's name), so the row stays one line. The newline's
    /// blank hangs past the row's end, so it needs none. Notes how far short of the row's end
    /// the last share then ends (`Row::rule_short`), where the rule is drawn to.
    pub(super) fn fit_rule(&mut self, rule: Range<usize>) {
        let row = &mut self.row;
        let tail: f32 = row.spans[rule.end..]
            .iter()
            .map(|span| match span.kind {
                SpanKind::Text => text_width(
                    row.display.get(span.range.clone()).unwrap_or_default(),
                    span.style.font,
                ),
                SpanKind::Space(gap) => gap.px.max(0.0),
                SpanKind::Blank(_) | SpanKind::Inline(_) => 0.0,
            })
            .sum();
        let gaps: Vec<&mut GapWidth> = row.spans[rule]
            .iter_mut()
            .filter_map(|span| match &mut span.kind {
                SpanKind::Space(gap) => Some(gap),
                _ => None,
            })
            .collect();
        if gaps.is_empty() {
            return;
        }
        let each = tail / gaps.len() as f32;
        let count = gaps.len() as f32;
        for gap in gaps {
            gap.px -= each;
        }
        row.rule_short = count * RULE_SLACK + tail;
    }
}
