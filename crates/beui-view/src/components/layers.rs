pub use crate::core::layers::Layer;
use crate::reactive::Children;
use crate::reactive::NodeId;
use beui_macros::component;

#[component]
pub fn Layers(children: Children<Layer>) -> NodeId {
    crate::core::layers(children)
}
