use crate::reactive::NodeId;
use crate::types::{FontId, Vec2};
use beui_tree::reactive::ReadSignal;

pub use crate::core::TextGeometry;
pub use beui_types::layout::Measured;
pub use beui_types::text::LineBox;

pub fn measure(node: NodeId) -> ReadSignal<Option<Measured>> {
    crate::core::measure(node)
}

pub fn measure_text(node: NodeId) -> ReadSignal<Option<TextGeometry>> {
    crate::core::measure_text(node)
}

pub fn measure_line(text: &str, font: FontId) -> Option<Vec2> {
    crate::core::measure_line(text, font)
}
