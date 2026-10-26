use beui_core::base::drawing::Draw;
use beui_core::geometry::Vec2;
use beui_core::node::NodeId;
use beui_macros::component;
use crate::reactive::Prop;

#[component]
pub fn Drawing(
    draw: Prop<Draw>,
    #[prop(default = None)] size: Prop<Option<Vec2>>
) -> NodeId {
    crate::core::drawing(draw, size)
}
