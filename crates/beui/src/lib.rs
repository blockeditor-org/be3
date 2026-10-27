extern crate self as beui;

pub use accesskit;
pub use beui_components_styled as styled;
pub use beui_components_unstyled as unstyled;
pub use beui_components_unstyled::datetime;
pub use beui_view::animation;
pub use beui_view::components::selectable;
pub use beui_view::primitives;
pub use beui_view::types::*;
pub use beui_view::window::{Window, WindowHandle, opens_windows, use_window};
pub use beui_view::{CORE, Core, child_type, value_child_type};

pub use beui_dock as dock;
pub use beui_keyboard as keyboard;

pub mod reactive {
    pub use beui_view::reactive::*;
}

#[cfg(not(beui_html))]
pub mod native;

#[cfg(beui_html)]
pub mod html {
    pub use beui_view::running::{STYLES_ID, choose_theme, render_to_string, stylesheet};
    #[cfg(target_arch = "wasm32")]
    pub use beui_view::running::{automate, hydrate, run};
}

#[cfg(not(beui_html))]
#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod web;

#[cfg(not(beui_html))]
mod keyboard_host;

#[cfg(not(beui_html))]
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod runner;
#[cfg(not(beui_html))]
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod saved_preferences;

#[cfg(beui_html)]
mod html_paintings;

#[cfg(not(beui_html))]
#[cfg(test)]
#[allow(hidden_glob_reexports)]
use beui_core::{
    base, color, context, damage, draw, drawing, filter, flash, font, geometry, image, input,
    interact, node, painter, screen_simulation, sight,
};
#[cfg(not(beui_html))]
#[cfg(test)]
use beui_inspector::{self as inspector, mouse_simulation};
#[cfg(not(beui_html))]
#[cfg(test)]
use beui_renderer_wgpu as renderer;

#[cfg(not(beui_html))]
#[cfg(test)]
mod tests;

pub fn open_inspector() -> bool {
    inspector_opener::open()
}

#[cfg(not(beui_html))]
mod inspector_opener {
    pub(crate) fn open() -> bool {
        crate::native::with_document(crate::native::Document::open_inspector);
        true
    }
}

#[cfg(beui_html)]
mod inspector_opener {
    pub(crate) fn open() -> bool {
        false
    }
}
