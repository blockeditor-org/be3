pub mod actions;
pub mod animation;
pub mod canvas;
pub mod drag;
pub mod drawing;
pub mod files;
pub mod frame;
pub mod grid;
pub mod interactive;
pub mod layers;
pub mod list;
pub mod nodes;
pub mod offset;
pub mod overlay;
pub mod portal;
pub mod reactive;
pub mod root;
pub mod selectable;
pub mod text;
pub mod virtual_list;
pub mod window;

pub use beui_core_html::drawing::Drawing;
pub use beui_core_html::interactive::ForwardedInput;
pub use beui_core_html::measure::TextGeometry;
pub(crate) use beui_core_html::measure::{measure, measure_line, measure_text};
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
pub(crate) use text::gap;
pub(crate) use text::icon;
pub(crate) use text::line_break;
pub(crate) use text::span;
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

pub mod running {
    pub use super::nodes::Page;
    #[cfg(target_arch = "wasm32")]
    pub use beui_core_html::automate::automate;
    pub use beui_core_html::run::{BASE_STYLES, STYLES_ID, stylesheet};

    use crate::primitives::Root;

    pub fn render_to_string(root: impl FnOnce() -> Root) -> String {
        beui_core_html::run::render_to_string(|| root().child())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn run(element_id: &str, root: impl FnOnce() -> Root) -> Result<(), String> {
        beui_core_html::run::run(element_id, || root().child())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn hydrate(element_id: &str, root: impl FnOnce() -> Root) -> Result<(), String> {
        beui_core_html::run::hydrate(element_id, || root().child())
    }
}

#[cfg(test)]
mod tests;
