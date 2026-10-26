use crate::reactive::Child;
use crate::reactive::Prop;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Turn(
    #[prop(default = 0)] quarter_turns: Prop<u8>,
    #[prop(default = 0.0)] angle: Prop<f32>,
    children: Child,
) -> NodeId {
    crate::core::turn(quarter_turns, angle, children)
}
