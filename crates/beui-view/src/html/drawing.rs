use crate::reactive::Draw;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Vec2;
use beui_core_html::css;
use beui_core_html::document::{create, set_attribute, set_style};
use beui_core_html::run::PINK;

use super::bind;

pub fn drawing(draw: Prop<Draw>, size: Prop<Option<Vec2>>) -> NodeId {
    let _ = draw;
    let drawing = create("Drawing", "div");
    set_attribute(drawing, "title", Some("TODO: Drawing".to_owned()));
    set_style(drawing, "background-color", Some(PINK.to_owned()));
    bind(size, move |size| {
        let (width, height) = match size {
            Some(size) => (css::px(size.x), css::px(size.y)),
            None => ("100%".to_owned(), "100%".to_owned()),
        };
        set_style(drawing, "width", Some(width));
        set_style(drawing, "height", Some(height));
    });
    drawing
}
