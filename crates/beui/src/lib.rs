extern crate self as beui;

pub use accesskit;
pub use beui_components_styled as styled;
pub use beui_components_unstyled as unstyled;
pub use beui_components_unstyled::datetime;
pub use beui_view::animation;
pub use beui_view::components::selectable;
pub use beui_view::escape;
pub use beui_view::primitives;
pub use beui_view::types::*;
pub use beui_view::window::{
    Window, WindowHandle, opens_windows, set_work_area, use_window, window_origin,
};
pub use beui_view::{CORE, Core, child_type, value_child_type};

pub use beui_dock as dock;
pub use beui_dock_host as dock_host;
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

#[cfg(beui_html)]
mod html_paintings;

/// Whether `open_inspector` opens anything: the inspector reads beui's native
/// document, so on the HTML core it cannot, and the browser's developer tools serve
/// instead.
pub const fn can_open_inspector() -> bool {
    cfg!(not(beui_html))
}

/// Opens the inspector over the running program, where `can_open_inspector` says it can.
pub fn open_inspector() {
    #[cfg(not(beui_html))]
    native::with_document(native::Document::open_inspector);
}
