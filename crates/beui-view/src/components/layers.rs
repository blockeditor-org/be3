use crate::reactive::{
    BuildsNode, ChildSegment, ChildValue, Children, ComponentContext, IntoChild, IntoSegment,
    NodeSlot, Scope, SlotChild, with_document,
};
use beui_core::base::layers::LayersNode;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_core::tree::remove_stored_node;
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
    fn adopt_scope(&mut self, scope: Scope) {
        self.node.adopt_scope(scope);
    }

    fn finish_component(&mut self, component: &ComponentContext, scope: &Scope) {
        self.node.finish_component(component, scope);
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

    fn store(self) -> NodeId {
        self.node
    }

    fn discard(stored: &NodeId) {
        remove_stored_node(*stored);
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
