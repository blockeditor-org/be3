use beui_macros::component;

use crate::reactive::{Prop, create_effect, with_document};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::geometry::Pos2;
use beui_core::node::NodeId;

#[component]
pub fn Stroke(
    from: Prop<Pos2>,
    to: Prop<Pos2>,
    #[prop(default = 1.0)] width: Prop<f32>,
    #[prop(default = Color32::WHITE)] color: Prop<Color32>,
) -> NodeId {
    let stroke = with_document(Document::create_stroke);
    create_effect(move || {
        let (from, to) = (from.get(), to.get());
        with_document(|document| document.set_stroke_ends(stroke, from, to));
    });
    create_effect(move || {
        let width = width.get();
        with_document(|document| document.set_stroke_width(stroke, width));
    });
    create_effect(move || {
        let color = color.get();
        with_document(|document| document.set_stroke_color(stroke, color));
    });
    stroke
}
