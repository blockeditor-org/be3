use std::ops::Range;

use beui_view::types::{Color, GapWidth, SpanStyle};

use super::table::text_width;
use super::{Builder, SpanKind};

impl Builder<'_> {
    /// A thematic break's characters, unseen: the rule is drawn in their place. Each takes an
    /// equal share of the row's width when `shared` (a rule with no marker before it), so the
    /// caret's stops divide the rule evenly and it steps across a character at a time.
    pub(super) fn rule_row(&mut self, content: Range<usize>, shared: bool, body: SpanStyle) {
        let bytes = &self.inputs.snapshot.bytes;
        let hidden = SpanStyle {
            color: Color::TRANSPARENT,
            underline: false,
            strikethrough: false,
            ..body
        };
        let share = 1.0 / content.len().max(1) as f32;
        for at in content {
            let text = String::from_utf8_lossy(&bytes[at..at + 1]).into_owned();
            let width = text_width(&text, hidden.font);
            self.push(&text, at..at + 1, true, hidden, SpanKind::Text);
            if shared {
                self.gap(GapWidth { px: -width, share }, body);
            }
        }
    }
}
