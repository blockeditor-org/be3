use crate::reactive::{ReadSignal, Render, child_scope, create_signal, with_document};
use beui_core::geometry::{Rect, Vec2};
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn LayoutBuilder(#[prop(children)] content: Render<ReadSignal<Vec2>>) -> NodeId {
    let size = create_signal(Vec2::ZERO);
    let builder = with_document(|document| document.create_layout_builder(size));
    let (child, context) = child_scope(|| content.call(size.read_only()));
    with_document(|document| {
        document.set_layout_builder_child(builder, child);
        document.set_layout_builder_content(builder, context);
    });
    builder.id()
}

#[component]
pub fn PlacementBuilder(#[prop(children)] content: Render<ReadSignal<Rect>>) -> NodeId {
    let (builder, placed) = with_document(beui_core::document::Document::create_placement_builder);
    let child = content.call(placed);
    with_document(|document| document.set_layout_builder_child(builder, child));
    builder.id()
}
