use crate::reactive::{ReadSignal, Render, create_signal, with_document};
use beui_core::geometry::{Rect, Vec2};
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn LayoutBuilder(#[prop(children)] content: Render<ReadSignal<Vec2>>) -> NodeId {
    let (size, set_size) = create_signal(Vec2::ZERO);
    let builder = with_document(|document| document.create_layout_builder(size.clone(), set_size));
    let child = content.call(size);
    with_document(|document| document.set_layout_builder_child(builder, child));
    builder.id()
}

#[component]
pub fn PlacementBuilder(#[prop(children)] content: Render<ReadSignal<Rect>>) -> NodeId {
    let (placed, set_placed) = create_signal(Rect::ZERO);
    let builder =
        with_document(|document| document.create_placement_builder(placed.clone(), set_placed));
    let child = content.call(placed);
    with_document(|document| document.set_layout_builder_child(builder, child));
    builder.id()
}
