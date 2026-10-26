pub use crate::core::layers::Layer;
use crate::reactive::Children;
use beui_core::node::NodeId;
use beui_macros::component;

#[component]
pub fn Layers(children: Children<Layer>) -> NodeId {
    crate::core::layers(children)
}
