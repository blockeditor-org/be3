use crate::reactive::NodeId;
use crate::reactive::Prop;
use beui_core_html::document::set_style;
use beui_core_html::run::PINK;

pub fn portal(node: Prop<Option<NodeId>>) -> NodeId {
    let _ = node;
    let portal = super::stand_in("Portal");
    for (property, value) in [
        ("min-width", "16px"),
        ("min-height", "16px"),
        ("background-color", PINK),
    ] {
        set_style(portal, property, Some(value.to_owned()));
    }
    portal
}
