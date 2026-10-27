use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::on_cleanup;
use beui_core_html::document::{create, set_style};

use super::bind;

pub(crate) fn portal(node: Prop<Option<NodeId>>) -> NodeId {
    let element = create("Portal", "div");
    set_style(element, "display", Some("grid".to_owned()));
    set_style(
        element,
        "grid-template-columns",
        Some("minmax(0, 1fr)".to_owned()),
    );
    let portal = beui_core_html::portal::portal(element);
    let held = portal.clone();
    bind(node, move |node| held.hold(node));
    on_cleanup(move || portal.release());
    element
}
