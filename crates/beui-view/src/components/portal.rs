use beui_core::node::NodeId;
use beui_macros::component;
use crate::reactive::Prop;

#[component]
pub fn Portal(
    #[prop(default = None)] node: Prop<Option<NodeId>>
) -> NodeId {
    crate::core::portal(node)
}
