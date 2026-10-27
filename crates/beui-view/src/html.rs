pub(crate) mod actions;
pub(crate) mod animation;
pub(crate) mod canvas;
pub(crate) mod colors;
pub(crate) mod drag;
pub(crate) mod drawing;
pub(crate) mod element;
pub(crate) mod files;
pub(crate) mod frame;
pub(crate) mod grid;
pub(crate) mod interactive;
pub(crate) mod layers;
pub(crate) mod list;
pub(crate) mod measure {
    pub(crate) use beui_core_html::measure::{TextGeometry, measure, measure_text};
}
pub(crate) mod nodes;
pub(crate) mod offset;
pub(crate) mod overlay;
pub(crate) mod painter {
    pub(crate) use beui_core_html::drawing::Painter;
    pub(crate) use beui_core_html::text_block::{TextBlock, lay_out_text};

    pub(crate) fn lay_out(
        _painter: &Painter,
        text: &str,
        font: beui_types::FontId,
        layout: beui_types::TextLayout,
    ) -> TextBlock {
        beui_core_html::text_block::lay_out_text(text, font, layout)
            .unwrap_or_else(|| TextBlock::empty(text, font, layout))
    }

    pub(crate) fn draw_text(
        painter: &Painter,
        origin: beui_types::Pos2,
        block: &TextBlock,
        color: beui_types::Color,
    ) {
        painter.draw_text(origin, block, color);
    }
}
pub(crate) mod portal;
pub(crate) mod reactive;
pub(crate) mod root;
pub(crate) mod selectable;
pub(crate) mod text;
pub(crate) mod virtual_list;
pub(crate) mod window;

pub(crate) use beui_core_html::tree;
pub(crate) use canvas::canvas;
pub(crate) use canvas::canvas_item;
pub(crate) use drawing::drawing;
pub(crate) use frame::frame;
pub(crate) use grid::grid;
pub(crate) use grid::grid_cell;
pub(crate) use interactive::interactive;
pub(crate) use layers::layers;
pub(crate) use list::list;
pub(crate) use list::spacer;
pub(crate) use offset::offset;
pub(crate) use overlay::overlay;
pub(crate) use portal::portal;
pub(crate) use root::root;
pub(crate) use text::text;
pub(crate) use text::text_item;
pub(crate) use virtual_list::virtual_list;
pub(crate) use window::{placed_windows, request_window};

use crate::reactive::{Prop, create_binding};

fn bind<T: Clone + 'static>(prop: Prop<T>, apply: impl Fn(T) + 'static) {
    match prop {
        Prop::Static(value) => apply(value),
        Prop::Dynamic(read) => {
            create_binding(move || apply(read.get()));
        }
    }
}

#[cfg(test)]
mod tests;
