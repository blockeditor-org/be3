use crate::reactive::Child;
use crate::reactive::Prop;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Turn(#[prop(default = 0)] quarter_turns: Prop<u8>, children: Child) -> NodeId {
    crate::core::turn(quarter_turns, children)
}
