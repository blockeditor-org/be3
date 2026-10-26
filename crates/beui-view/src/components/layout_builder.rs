use crate::reactive::ReadSignal;
use crate::reactive::Render;
use beui_core::geometry::Rect;
use beui_core::geometry::Vec2;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn LayoutBuilder(#[prop(children)] content: Render<ReadSignal<Vec2>>) -> NodeId {
    crate::core::layout_builder(content)
}

#[component]
pub fn PlacementBuilder(#[prop(children)] content: Render<ReadSignal<Rect>>) -> NodeId {
    crate::core::placement_builder(content)
}
