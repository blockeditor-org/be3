use beui_tree::reactive::ReadSignal;
use beui_types::NodeId;
use beui_types::layout::Measured;

use beui_core::current::with_document;

pub(crate) use beui_core::base::text::TextGeometry;

pub(crate) fn measure(node: NodeId) -> ReadSignal<Option<Measured>> {
    with_document(|document| document.watch_measure(node))
}

pub(crate) fn scrolled_by(node: NodeId) -> beui_types::Vec2 {
    with_document(|document| document.scrolled_by(node))
}

pub(crate) fn measure_text(node: NodeId) -> ReadSignal<Option<TextGeometry>> {
    with_document(|document| document.watch_measure_text(node))
}
