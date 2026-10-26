use crate::reactive::Child;
use crate::reactive::Prop;
use beui_core::base::Sides;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Fade(
    #[prop(default = Sides::default())] edges: Prop<Sides>,
    #[prop(default = 1.0)] opacity: Prop<f32>,
    children: Child,
) -> NodeId {
    crate::core::fade(edges, opacity, children)
}
