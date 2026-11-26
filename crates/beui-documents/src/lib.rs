//! The beui facade without the components: the types, the reactive graph, views, windows and
//! the native document's API (`native`). The facade (`beui`) re-exports all of
//! it at the same paths and adds the components, the dock and the keyboard, so every crate
//! spells an item one way. The app runtime depends on this crate under the name `beui`
//! (decisions/beui_facade_excludes_the_engine.md), so an edit to a component does not
//! rebuild it.

pub use accesskit;
pub use beui_view::animation;
pub use beui_view::components::selectable;
pub use beui_view::escape;
pub use beui_view::primitives;
pub use beui_view::types::*;
pub use beui_view::window::{
    Window, WindowAnchor, WindowHandle, opens_windows, set_work_area, use_window, window_origin,
};
pub use beui_view::{CORE, Core, child_type, value_child_type};

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
