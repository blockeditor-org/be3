use crate::painter::TextBlock;
use crate::reactive::{NodeId, ReadSignal};
use crate::types::{FontId, Pos2, Rect, TextLayout, Vec2};
use beui_tree::reactive::Get;

pub use beui_types::layout::Measured;
pub use beui_types::text::LineBox;

/// A `Text`'s last layout, in window coordinates: a snapshot natively, live DOM
/// queries on HTML. Offsets are UTF-8 bytes of the text's content on every core:
/// a string or `Span` holds its bytes, a `glyphs` entry counts as the bytes it
/// covers, an `Icon`, `Gap` or `Break` holds none, an inline `TextItem` stands for
/// its `text` (U+FFFC when it has none) and one given `at` holds none
/// (`text_geometry_counts_utf8_bytes_and_only_the_bytes_inline_children_stand_for`).
#[derive(Clone, PartialEq, Debug)]
pub struct TextGeometry(crate::core::measure::TextGeometry);

impl TextGeometry {
    pub fn bounds(&self) -> Rect {
        self.0.bounds()
    }

    /// The scale of the space the text sits in: multiply a length of the text's own
    /// (a padding) by it before comparing with window positions.
    pub fn scale(&self) -> f32 {
        self.0.scale()
    }

    pub fn caret_rect(&self, index: usize, width: f32) -> Option<Rect> {
        self.0.caret_rect(index, width)
    }

    pub fn index_at(&self, pos: Pos2) -> Option<usize> {
        self.0.index_at(pos)
    }

    pub fn inline_at(&self, pos: Pos2) -> Option<usize> {
        self.0.inline_at(pos)
    }

    pub fn lines(&self) -> Vec<LineBox> {
        self.0.lines()
    }
}

/// Where `node` was laid out. `None` until it is first laid out and again once it
/// is unplaced, and on the HTML core (or under `delay_measurement`) a frame after
/// its cause, so callers must handle a late answer. One signal per node, made on
/// the first call and dropped with the node, so calling it repeatedly is cheap.
pub fn measure(node: NodeId) -> ReadSignal<Option<Measured>> {
    crate::core::measure::measure(node)
}

/// How far the scrolls around `node` scrolled it when it was last laid out: added to where
/// it was measured, it gives a place a scroll does not move.
pub fn scrolled_by(node: NodeId) -> Vec2 {
    crate::core::measure::scrolled_by(node)
}

#[derive(Clone, Copy)]
pub struct TextGeometrySignal(ReadSignal<Option<crate::core::measure::TextGeometry>>);

impl Get<Option<TextGeometry>> for TextGeometrySignal {
    fn with<R>(&self, f: impl FnOnce(&Option<TextGeometry>) -> R) -> R {
        let geometry = self.0.get().map(TextGeometry);
        f(&geometry)
    }
}

/// As `measure`, for a `Text`'s geometry; stays `None` for any other node.
pub fn measure_text(node: NodeId) -> TextGeometrySignal {
    TextGeometrySignal(crate::core::measure::measure_text(node))
}

/// Lays out text that is not in the tree, answering at once on both cores (the
/// HTML core from the browser's font metrics); `None` before the document has
/// fonts. Read `use_font_revision()` and `pixels_per_point()` in the memo that
/// calls it, so it measures again when fonts load or the scale changes.
pub fn lay_out_text(text: &str, font: FontId, layout: TextLayout) -> Option<TextBlock> {
    crate::core::painter::lay_out_text(text, font, layout).map(TextBlock)
}
