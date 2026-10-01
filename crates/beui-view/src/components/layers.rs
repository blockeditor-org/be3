use crate::reactive::{
    BuildsNode, ChildSegment, ChildValue, Children, IntoChild, IntoSegment, NodeSlot, Scope,
    SlotChild, with_document,
};
use beui_core::base::layers::LayersNode;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_macros::component;

pub struct Layer {
    node: NodeId,
}

impl BuildsNode for Layer {
    fn built_node(&self) -> NodeId {
        self.node
    }
}

impl ChildValue for Layer {
    fn anchor(&self) -> Option<NodeId> {
        Some(self.node)
    }

    fn adopt_scope(&mut self, scope: Scope) {
        self.node.adopt_scope(scope);
    }
}

crate::child_type!(Layer);

impl IntoChild<Layer> for NodeId {
    fn into_child(self) -> Layer {
        Layer { node: self }
    }
}

impl IntoSegment<Layer> for NodeId {
    fn into_segment(self) -> ChildSegment<Layer> {
        ChildSegment::One(self.into_child())
    }
}

impl SlotChild for Layer {
    type Stored = NodeId;

    fn store(self, _parent: Option<NodeId>) -> NodeId {
        self.node
    }

    fn stored_node(stored: &NodeId) -> Option<NodeId> {
        Some(*stored)
    }
}

impl NodeSlot for Layer {
    type Host = LayersNode;
}

#[component]
pub fn Layers(children: Children<Layer>) -> NodeId {
    let layers = with_document(Document::create_layers);
    children.mount(layers);
    layers.id()
}
