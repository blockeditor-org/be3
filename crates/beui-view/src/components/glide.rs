use crate::reactive::{Child, NodeId, Prop};
use crate::types::{GlideHold, Pos2, Vec2};
use beui_macros::component;

#[component]
pub fn Glide(
    #[prop(default = 0)] arm: Prop<u64>,
    #[prop(default = None)] from: Prop<Option<Pos2>>,
    children: Child,
) -> NodeId {
    crate::core::glide::glide(arm, from, children)
}

pub fn hold_glide(glide: NodeId, hold: Option<GlideHold>) {
    crate::core::glide::hold_glide(glide, hold);
}

pub fn glide_displacement(glide: NodeId) -> Option<Vec2> {
    crate::core::glide::glide_displacement(glide)
}

pub fn set_raised(node: NodeId, raised: bool) {
    crate::core::glide::set_raised(node, raised);
}
