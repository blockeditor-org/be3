use beui_macros::component;

use crate::reactive::{Prop, create_effect, with_document};
use beui_core::base::drawing::Draw;
use beui_core::document::Document;
use beui_core::node::NodeId;

#[component]
pub fn Drawing(draw: Prop<Draw>) -> NodeId {
    let drawing = with_document(Document::create_drawing);
    create_effect(move || {
        let draw = draw.get();
        with_document(|document| document.set_drawing(drawing, draw));
    });
    drawing.id()
}
