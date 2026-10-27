use crate::reactive::{NodeId, ReadSignal};
use crate::types::{FontId, Pos2, Rect, Vec2};
use beui_tree::reactive::Get;

pub use beui_types::layout::Measured;
pub use beui_types::text::LineBox;

#[derive(Clone, PartialEq, Debug)]
pub struct TextGeometry(crate::core::measure::TextGeometry);

impl TextGeometry {
    pub fn bounds(&self) -> Rect {
        self.0.bounds()
    }

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

pub fn measure(node: NodeId) -> ReadSignal<Option<Measured>> {
    crate::core::measure::measure(node)
}

#[derive(Clone, Copy)]
pub struct TextGeometrySignal(ReadSignal<Option<crate::core::measure::TextGeometry>>);

impl Get<Option<TextGeometry>> for TextGeometrySignal {
    fn with<R>(&self, f: impl FnOnce(&Option<TextGeometry>) -> R) -> R {
        let geometry = self.0.get().map(TextGeometry);
        f(&geometry)
    }
}

pub fn measure_text(node: NodeId) -> TextGeometrySignal {
    TextGeometrySignal(crate::core::measure::measure_text(node))
}

pub fn measure_line(text: &str, font: FontId) -> Option<Vec2> {
    crate::core::measure::measure_line(text, font)
}

pub fn draws(text: &str, font: FontId) -> bool {
    crate::core::measure::draws(text, font)
}
