use beui_macros::component;

use crate::reactive::{Prop, create_effect, on_cleanup, with_document};
use beui_core::document::Document;
use beui_core::node::NodeId;

#[component]
pub fn Portal(#[prop(default = None)] node: Prop<Option<NodeId>>) -> NodeId {
    let portal = with_document(Document::create_portal);
    create_effect(move || {
        let node = node.get();
        with_document(|document| document.set_portal_child(portal, node));
    });
    on_cleanup(move || {
        beui_core::current::try_with_document(|document| document.set_portal_child(portal, None));
    });
    portal.id()
}
