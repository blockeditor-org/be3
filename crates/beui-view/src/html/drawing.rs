use std::cell::RefCell;
use std::rc::Rc;

use crate::painter::{Draw, Painter};
use crate::reactive::Get;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::{create_binding, create_memo};
use crate::types::Vec2;
use beui_core_html::css;
use beui_core_html::document::{create, set_markup, set_style};
use beui_core_html::drawing::{Images, draw_svg};

use super::bind;

pub(crate) fn drawing(draw: Prop<Draw>, size: Prop<Option<Vec2>>) -> NodeId {
    let drawing = create("Drawing", "div");
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
    let measured = super::measure::measure(drawing);
    let area = create_memo(move || {
        let size = size.get();
        let room = measured.get().map(|measured| measured.size).or(size)?;
        (room.x > 0.0 && room.y > 0.0).then_some((room, size.is_some()))
    });
    let ids = format!("beui-drawing{}-{}-", drawing.index(), drawing.generation());
    let images = Rc::new(RefCell::new(Images::default()));
    create_binding(move || {
        let markup = area.get().map(|(room, clipped)| {
            let draw = draw.get();
            let draw: beui_core_html::drawing::Draw =
                Rc::new(move |painter, rect| draw(&Painter::of(painter), rect));
            draw_svg(&draw, room, clipped, &ids, &mut images.borrow_mut())
        });
        set_markup(drawing, markup);
    });
    drawing
}
