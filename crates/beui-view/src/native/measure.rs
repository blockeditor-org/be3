use beui_core::font::TextLayout;
use beui_core::geometry::{Rect, Vec2};
use beui_core::node::NodeId;
use beui_tree::reactive::ReadSignal;
use beui_types::text::FontId;
use beui_types::layout::Measured;

use beui_core::current::{try_with_document, with_document};

pub use beui_core::base::text::TextGeometry;

pub fn measure(node: NodeId) -> ReadSignal<Option<Measured>> {
    with_document(|document| document.watch_measure(node))
}

pub fn measure_text(node: NodeId) -> ReadSignal<Option<TextGeometry>> {
    with_document(|document| document.watch_measure_text(node))
}

pub fn measure_line(text: &str, font: FontId) -> Option<Vec2> {
    try_with_document(|document| document.layout_text(text, font, TextLayout::DEFAULT))
        .flatten()
        .map(|galley| Vec2::new(galley.size().x, galley.line_height()))
}
