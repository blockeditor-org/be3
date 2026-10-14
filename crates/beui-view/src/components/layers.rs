use crate::reactive::{Children, Placed, with_document};
use beui_core::base::layers::LayersNode;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_macros::component;

pub type Layer = Placed<LayersNode>;

#[component]
pub fn Layers(children: Children<Layer>) -> NodeId {
    let layers = with_document(Document::create_layers);
    children.mount(layers);
    layers.id()
}
