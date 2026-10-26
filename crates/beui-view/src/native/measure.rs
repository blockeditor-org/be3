use beui_core::geometry::Rect;
use beui_core::node::NodeId;
use beui_tree::reactive::ReadSignal;

use beui_core::current::with_document;

pub use beui_core::base::text::TextGeometry;

pub fn measure(node: NodeId) -> ReadSignal<Option<Rect>> {
    with_document(|document| document.watch_measure(node))
}

pub fn measure_text(node: NodeId) -> ReadSignal<Option<TextGeometry>> {
    with_document(|document| document.watch_measure_text(node))
}
