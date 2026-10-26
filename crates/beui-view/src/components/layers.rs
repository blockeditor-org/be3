use beui_core::node::NodeId;
use beui_macros::component;
use crate::reactive::Children;
pub use crate::core::layers::{Layer};

#[component]
pub fn Layers(
    children: Children<Layer>
) -> NodeId {
    crate::core::layers(children)
}
