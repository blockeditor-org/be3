use crate::reactive::Child;
use crate::reactive::Prop;
use beui_core::base::embed::EmbedSlot;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Embed(
    slot: EmbedSlot,
    #[prop(default = true)] punch: Prop<bool>,
    #[prop(default = 0.0)] rotation: Prop<f32>,
    children: Option<Child>,
) -> NodeId {
    crate::core::embed(slot, punch, rotation, children)
}
