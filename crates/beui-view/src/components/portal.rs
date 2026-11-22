use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::component;

#[component]
pub fn Portal(#[prop(default = None)] node: Prop<Option<NodeId>>) -> NodeId {
    crate::core::portal(node)
}
