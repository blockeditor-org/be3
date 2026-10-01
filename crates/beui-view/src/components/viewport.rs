use beui_macros::component;

use crate::reactive::{Prop, create_effect, with_document};
use beui_core::document::Document;
use beui_core::drawing::Drawing;
use beui_core::node::NodeId;

#[component]
pub fn Viewport(drawing: Prop<Option<Drawing>>) -> NodeId {
    let viewport = with_document(Document::create_viewport);
    create_effect(move || {
        let drawing = drawing.get();
        with_document(|document| document.set_viewport_drawing(viewport, drawing));
    });
    viewport.id()
}
