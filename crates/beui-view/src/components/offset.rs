use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::Prop;
use beui_core::base::list::Direction;
use beui_core::base::offset::ScrollPosition;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Offset(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(default = false)] fit: bool,
    #[prop(default = false)] follow_end: Prop<bool>,
    on_change: Callback<ScrollPosition>,
    children: Child,
) -> NodeId {
    crate::core::offset(offset, direction, fit, follow_end, on_change, children)
}
