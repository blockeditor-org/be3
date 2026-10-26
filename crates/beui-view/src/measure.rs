use crate::reactive::NodeId;
use crate::types::Rect;
use beui_tree::reactive::ReadSignal;

pub use crate::core::TextGeometry;
pub use beui_types::text::LineBox;

pub fn measure(node: NodeId) -> ReadSignal<Option<Rect>> {
    crate::core::measure(node)
}

pub fn measure_text(node: NodeId) -> ReadSignal<Option<TextGeometry>> {
    crate::core::measure_text(node)
}
