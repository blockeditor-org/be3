use crate::reactive::Children;
use crate::reactive::NodeId;
pub use crate::types::tree::Layer;
use beui_macros::component;

/// Lays every child over the same box, sized to the largest, later children on top.
/// Pointer input reaches only the topmost catcher under the pointer (and the catchers
/// around it), never one in a layer beneath.
#[component]
pub fn Layers(children: Children<Layer>) -> NodeId {
    crate::core::layers(children)
}
