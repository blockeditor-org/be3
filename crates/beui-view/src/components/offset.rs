use crate::reactive::Callback;
use crate::reactive::Child;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Direction;
use crate::types::ScrollPosition;
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
