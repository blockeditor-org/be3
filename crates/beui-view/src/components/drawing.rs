use crate::reactive::Draw;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Vec2;
use beui_macros::component;

#[component]
pub fn Drawing(draw: Prop<Draw>, #[prop(default = None)] size: Prop<Option<Vec2>>) -> NodeId {
    crate::core::drawing(draw, size)
}
