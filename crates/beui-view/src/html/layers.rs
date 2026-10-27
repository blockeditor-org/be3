use crate::reactive::Children;
use crate::reactive::NodeId;
use crate::types::tree::Layer;
use beui_core_html::document::{create, set_attribute, set_style};
use beui_core_html::tree::{LayersKind, host};

pub(crate) fn layers(children: Children<Layer>) -> NodeId {
    let layers = create("Layers", "div");
    set_attribute(layers, "class", Some("beui-layers".to_owned()));
    set_style(layers, "display", Some("grid".to_owned()));
    set_style(
        layers,
        "grid-template",
        Some("minmax(0, 1fr) / minmax(0, 1fr)".to_owned()),
    );
    children.mount(host::<LayersKind>(layers));
    layers
}
