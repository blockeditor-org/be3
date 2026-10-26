use crate::reactive::Get;
use crate::reactive::Prop;
use crate::reactive::create_effect;
use crate::reactive::with_document;
use beui_core::base::drawing::Draw;
use beui_core::document::Document;
use beui_core::geometry::Vec2;
use beui_core::node::NodeId;

pub fn drawing(
    draw: Prop<Draw>,
    size: Prop<Option<Vec2>>
) -> NodeId {
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
