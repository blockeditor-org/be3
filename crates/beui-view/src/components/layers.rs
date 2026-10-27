use crate::reactive::Children;
use crate::reactive::NodeId;
pub use crate::types::tree::Layer;
use beui_macros::component;

#[component]
pub fn Layers(children: Children<Layer>) -> NodeId {
    crate::core::layers(children)
}
