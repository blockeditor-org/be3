use crate::reactive::ReadSignal;
use crate::reactive::Render;
use crate::reactive::child_scope;
use crate::reactive::create_signal;
use crate::reactive::with_document;
use beui_core::geometry::Rect;
use beui_core::geometry::Vec2;
use beui_core::node::NodeId;

pub fn layout_builder(content: Render<ReadSignal<Vec2>>) -> NodeId {
    let size = create_signal(Vec2::ZERO);
    let builder = with_document(|document| document.create_layout_builder(size));
    let (child, context) = child_scope(|| content.call(size.read_only()));
    with_document(|document| {
        document.set_layout_builder_child(builder, child);
        document.set_layout_builder_content(builder, context);
    });
    builder.id()
}

pub fn placement_builder(content: Render<ReadSignal<Rect>>) -> NodeId {
    let (builder, placed) = with_document(beui_core::document::Document::create_placement_builder);
    let child = content.call(placed);
    with_document(|document| document.set_layout_builder_child(builder, child));
    builder.id()
}
