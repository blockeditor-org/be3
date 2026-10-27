use crate::reactive::Children;
use crate::types::tree::Layer;
use beui_core::current::with_document;
use beui_core::document::Document;
use beui_core::node::NodeId;

pub(crate) fn layers(children: Children<Layer>) -> NodeId {
    let layers = with_document(Document::create_layers);
    children.mount(layers);
    layers.id()
}
