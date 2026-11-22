use crate::reactive::Draw;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::component;
use crate::types::Vec2;

#[component]
pub fn Drawing(draw: Prop<Draw>, #[prop(default = None)] size: Prop<Option<Vec2>>) -> NodeId {
    crate::core::drawing(draw, size)
}
