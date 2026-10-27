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
        let (width, height, ratio, max_width) = match size {
            Some(size) if size.x > 0.0 && size.y > 0.0 => (
                css::px(size.x),
                "auto".to_owned(),
                Some(format!("{} / {}", css::number(size.x), css::number(size.y))),
                Some("100%".to_owned()),
            ),
            Some(size) => (css::px(size.x), css::px(size.y), None, None),
            None => ("100%".to_owned(), "100%".to_owned(), None, None),
        };
        set_style(drawing, "width", Some(width));
        set_style(drawing, "height", Some(height));
        set_style(drawing, "aspect-ratio", ratio);
        set_style(drawing, "max-width", max_width);
    });
    drawing
}
