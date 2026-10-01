use beui_macros::component;

use crate::reactive::{Prop, create_effect, with_document};
use beui_core::base::drawing::Draw;
use beui_core::document::Document;
use beui_core::geometry::Vec2;
use beui_core::node::NodeId;

#[component]
pub fn Drawing(draw: Prop<Draw>, #[prop(default = None)] size: Prop<Option<Vec2>>) -> NodeId {
    let drawing = with_document(Document::create_drawing);
    create_effect(move || {
        let draw = draw.get();
        with_document(|document| document.set_drawing(drawing, draw));
    });
    create_effect(move || {
        let size = size.get();
        with_document(|document| document.set_drawing_size(drawing, size));
    });
    drawing.id()
}
