use crate::reactive::Child;
use crate::reactive::Prop;
use beui_core::geometry::Vec2;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Shift(by: Prop<Vec2>, children: Child) -> NodeId {
    crate::core::shift(by, children)
}
