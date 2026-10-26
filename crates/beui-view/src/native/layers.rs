use crate::reactive::Children;
use crate::reactive::Placed;
use crate::reactive::with_document;
use beui_core::base::layers::LayersNode;
use beui_core::document::Document;
use beui_core::node::NodeId;

pub type Layer = Placed<LayersNode>;

pub fn layers(children: Children<Layer>) -> NodeId {
    let layers = with_document(Document::create_layers);
    children.mount(layers);
    layers.id()
}
